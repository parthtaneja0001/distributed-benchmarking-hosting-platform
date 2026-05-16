package main

import (
    "encoding/json"
    "log"
    "net/http"

    "github.com/gorilla/websocket"
)

var upgrader = websocket.Upgrader{
    CheckOrigin: func(r *http.Request) bool { return true },
}

// Order is what the bot sends
type Order struct {
    OrderID   string  `json:"order_id"`
    Type      string  `json:"type"`   // limit, market, cancel
    Side      string  `json:"side"`   // buy, sell
    Price     float64 `json:"price"`
    Quantity  int     `json:"quantity"`
    Timestamp int64   `json:"timestamp"` // microseconds or milliseconds
}

// Ack is sent back immediately for every order
type Ack struct {
    Event     string `json:"event"`
    OrderID   string `json:"order_id"`
    Timestamp int64  `json:"timestamp"`
}

// Fill is sent back (simulated) for every limit/market order
type Fill struct {
    Event    string  `json:"event"`
    OrderID  string  `json:"order_id"`
    Price    float64 `json:"price"`
    Quantity int     `json:"quantity"`
}

func handleConnection(w http.ResponseWriter, r *http.Request) {
    conn, err := upgrader.Upgrade(w, r, nil)
    if err != nil {
        log.Printf("Upgrade error: %v", err)
        return
    }
    defer conn.Close()

    log.Printf("Bot connected from %s", r.RemoteAddr)

    for {
        _, message, err := conn.ReadMessage()
        if err != nil {
            log.Printf("Read error: %v", err)
            break
        }

        var order Order
        if err := json.Unmarshal(message, &order); err != nil {
            log.Printf("Invalid order: %v", err)
            continue
        }

        // 1) Immediately send an acknowledgement with the original timestamp
        ack := Ack{
            Event:     "ack",
            OrderID:   order.OrderID,
            Timestamp: order.Timestamp,
        }
        ackBytes, _ := json.Marshal(ack)
        if err := conn.WriteMessage(websocket.TextMessage, ackBytes); err != nil {
            log.Printf("Write ack error: %v", err)
            break
        }

        // 2) Simulate a fill (for limit/market orders)
        if order.Type == "limit" || order.Type == "market" {
            // Fake fill: always fills completely at the requested price
            fill := Fill{
                Event:    "fill",
                OrderID:  order.OrderID,
                Price:    order.Price,
                Quantity: order.Quantity,
            }
            fillBytes, _ := json.Marshal(fill)
            if err := conn.WriteMessage(websocket.TextMessage, fillBytes); err != nil {
                log.Printf("Write fill error: %v", err)
                break
            }
        }

        // (For cancel orders, just the ack is enough for now)
        log.Printf("Processed order %s (%s)", order.OrderID, order.Type)
    }
}

func main() {
    http.HandleFunc("/ws", handleConnection)
    port := ":8080"
    log.Printf("Dummy matching engine listening on ws://localhost%s/ws", port)
    log.Fatal(http.ListenAndServe(port, nil))
}