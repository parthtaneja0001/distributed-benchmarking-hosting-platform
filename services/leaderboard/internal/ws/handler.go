package ws

import (
	"context"
	"log"
	"net/http"
	"time"

	"github.com/gorilla/websocket"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"
)

type LeaderboardSource interface {
	Current(ctx context.Context) (model.LeaderboardResponse, error)
}

type Handler struct {
	source   LeaderboardSource
	period   time.Duration
	upgrader websocket.Upgrader
}

func NewHandler(source LeaderboardSource, period time.Duration) *Handler {
	return &Handler{
		source: source,
		period: period,
		upgrader: websocket.Upgrader{
			// Local prototype accepts same-host browser connections. Origin policy
			// should be tightened when the frontend host is finalized.
			CheckOrigin: func(*http.Request) bool { return true },
		},
	}
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("leaderboard websocket upgrade failed: %v", err)
		return
	}
	defer conn.Close()

	ticker := time.NewTicker(h.period)
	defer ticker.Stop()

	// Send an immediate snapshot so clients do not wait for the first tick.
	if !h.writeSnapshot(r.Context(), conn) {
		return
	}

	for {
		select {
		case <-r.Context().Done():
			return
		case <-ticker.C:
			if !h.writeSnapshot(r.Context(), conn) {
				return
			}
		}
	}
}

func (h *Handler) writeSnapshot(ctx context.Context, conn *websocket.Conn) bool {
	snapshot, err := h.source.Current(ctx)
	if err != nil {
		log.Printf("leaderboard websocket snapshot failed: %v", err)
		return false
	}

	if err := conn.WriteJSON(snapshot); err != nil {
		log.Printf("leaderboard websocket write failed: %v", err)
		return false
	}

	return true
}
