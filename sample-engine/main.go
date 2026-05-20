package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"sort"
	"sync"

	"github.com/gorilla/websocket"
)

// ---------- data structures ----------

type Order struct {
	OrderID   string  `json:"order_id"`
	Type      string  `json:"type"`      // "limit" or "market" or "cancel"
	Side      string  `json:"side"`      // "buy" or "sell"
	Price     float64 `json:"price"`     // ignored for market / cancel
	Quantity  int     `json:"quantity"`  // ignored for cancel
	Timestamp int64   `json:"timestamp"` // microseconds since epoch, echoed in ack
}

type Ack struct {
	Event     string `json:"event"`
	OrderID   string `json:"order_id"`
	Timestamp int64  `json:"timestamp"`
}

type Fill struct {
	Event    string  `json:"event"`
	OrderID  string  `json:"order_id"`
	Price    float64 `json:"price"`
	Quantity int     `json:"quantity"`
}

// ---------- order book ----------

type bookEntry struct {
	order Order
	conn  *websocket.Conn
	mu    sync.Mutex
}

type priceLevel struct {
	orders []*bookEntry
}

type OrderBook struct {
	mu         sync.Mutex
	buyLevels  []priceLevel            // sorted descending price (best buy highest price)
	sellLevels []priceLevel            // sorted ascending price  (best sell lowest price)
	orderMap   map[string]*bookEntry  // orderID -> entry (for cancels)
}

func NewOrderBook() *OrderBook {
	return &OrderBook{
		orderMap: make(map[string]*bookEntry),
	}
}

// addOrder inserts an order into the correct side/price level.
func (ob *OrderBook) addOrder(o Order, conn *websocket.Conn) *bookEntry {
	entry := &bookEntry{order: o, conn: conn}
	ob.orderMap[o.OrderID] = entry

	levels := &ob.buyLevels
	if o.Side == "sell" {
		levels = &ob.sellLevels
	}

	// find or create price level
	var level *priceLevel
	for i := range *levels {
		if (*levels)[i].orders[0].order.Price == o.Price {
			level = &(*levels)[i]
			break
		}
	}
	if level == nil {
		*levels = append(*levels, priceLevel{})
		level = &(*levels)[len(*levels)-1]
		// insert order as first entry at this level (will be moved to correct position in FIFO order)
		level.orders = append(level.orders, entry)
		// sort levels: buys descending, sells ascending
		sort.Slice(*levels, func(i, j int) bool {
			if o.Side == "buy" {
				return (*levels)[i].orders[0].order.Price > (*levels)[j].orders[0].order.Price
			}
			return (*levels)[i].orders[0].order.Price < (*levels)[j].orders[0].order.Price
		})
	} else {
		level.orders = append(level.orders, entry)
	}
	return entry
}

// removeOrder removes an order from the book by ID.
func (ob *OrderBook) removeOrder(orderID string) *bookEntry {
	entry, ok := ob.orderMap[orderID]
	if !ok {
		return nil
	}
	delete(ob.orderMap, orderID)

	levels := &ob.buyLevels
	if entry.order.Side == "sell" {
		levels = &ob.sellLevels
	}
	for i := range *levels {
		lvl := &(*levels)[i]
		for j, e := range lvl.orders {
			if e == entry {
				lvl.orders = append(lvl.orders[:j], lvl.orders[j+1:]...)
				if len(lvl.orders) == 0 {
					*levels = append((*levels)[:i], (*levels)[i+1:]...)
				}
				return entry
			}
		}
	}
	return nil
}

// match runs after a new limit order is added; tries to cross the spread.
func (ob *OrderBook) match(o Order, conn *websocket.Conn) []Fill {
	var fills []Fill

	if o.Side == "buy" {
		for len(ob.sellLevels) > 0 && o.Quantity > 0 {
			bestSell := &ob.sellLevels[0]
			if bestSell.orders[0].order.Price > o.Price {
				break
			}
			entry := bestSell.orders[0]
			qty := min(o.Quantity, entry.order.Quantity)
			price := entry.order.Price

			fills = append(fills, Fill{
				Event:    "fill",
				OrderID:  o.OrderID,
				Price:    price,
				Quantity: qty,
			})
			fills = append(fills, Fill{
				Event:    "fill",
				OrderID:  entry.order.OrderID,
				Price:    price,
				Quantity: qty,
			})

			entry.order.Quantity -= qty
			o.Quantity -= qty

			if entry.order.Quantity == 0 {
				ob.removeOrder(entry.order.OrderID)
			}
		}
	} else { // sell side
		for len(ob.buyLevels) > 0 && o.Quantity > 0 {
			bestBuy := &ob.buyLevels[0]
			if bestBuy.orders[0].order.Price < o.Price {
				break
			}
			entry := bestBuy.orders[0]
			qty := min(o.Quantity, entry.order.Quantity)
			price := entry.order.Price

			fills = append(fills, Fill{
				Event:    "fill",
				OrderID:  o.OrderID,
				Price:    price,
				Quantity: qty,
			})
			fills = append(fills, Fill{
				Event:    "fill",
				OrderID:  entry.order.OrderID,
				Price:    price,
				Quantity: qty,
			})

			entry.order.Quantity -= qty
			o.Quantity -= qty

			if entry.order.Quantity == 0 {
				ob.removeOrder(entry.order.OrderID)
			}
		}
	}
	return fills
}

// ---------- WebSocket handler ----------

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

func handleConnection(book *OrderBook, conn *websocket.Conn) {
	defer conn.Close()

	for {
		_, msg, err := conn.ReadMessage()
		if err != nil {
			log.Printf("read error: %v", err)
			break
		}

		var order Order
		if err := json.Unmarshal(msg, &order); err != nil {
			log.Printf("invalid order: %v", err)
			continue
		}

		// always send ack (echo timestamp)
		ack := Ack{
			Event:     "ack",
			OrderID:   order.OrderID,
			Timestamp: order.Timestamp,
		}
		ackBytes, _ := json.Marshal(ack)
		conn.WriteMessage(websocket.TextMessage, ackBytes)

		switch order.Type {
		case "limit":
			book.mu.Lock()
			fills := book.match(order, conn)
			if order.Quantity > 0 {
				book.addOrder(order, conn)
			}
			book.mu.Unlock()

			for _, f := range fills {
				fillBytes, _ := json.Marshal(f)
				conn.WriteMessage(websocket.TextMessage, fillBytes)
			}

		case "market":
			book.mu.Lock()
			fills := book.match(order, conn)
			book.mu.Unlock()

			for _, f := range fills {
				fillBytes, _ := json.Marshal(f)
				conn.WriteMessage(websocket.TextMessage, fillBytes)
			}

		case "cancel":
			book.mu.Lock()
			entry := book.removeOrder(order.OrderID)
			book.mu.Unlock()
			if entry != nil {
				cancelAck := map[string]string{
					"event":    "cancel_ack",
					"order_id": order.OrderID,
				}
				ackB, _ := json.Marshal(cancelAck)
				conn.WriteMessage(websocket.TextMessage, ackB)
			}
		}
	}
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func main() {
	book := NewOrderBook()

	http.HandleFunc("/ws", func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Printf("upgrade error: %v", err)
			return
		}
		log.Printf("Bot connected from %s", r.RemoteAddr)
		handleConnection(book, conn)
	})

	port := 8080
	log.Printf("Sample matching engine listening on :%d/ws", port)
	log.Fatal(http.ListenAndServe(fmt.Sprintf(":%d", port), nil))
}