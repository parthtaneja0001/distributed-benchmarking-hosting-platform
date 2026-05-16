package httpapi

import (
	"context"
	"encoding/json"
	"net/http"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"
)

type LeaderboardSource interface {
	Current(ctx context.Context) (model.LeaderboardResponse, error)
}

type Handler struct {
	source LeaderboardSource
	ws     http.Handler
}

func NewHandler(source LeaderboardSource, ws http.Handler) *Handler {
	return &Handler{
		source: source,
		ws:     ws,
	}
}

func (h *Handler) Routes() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", h.health)
	mux.HandleFunc("GET /leaderboard", h.leaderboard)
	mux.Handle("GET /ws/leaderboard", h.ws)
	return mux
}

func (h *Handler) health(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (h *Handler) leaderboard(w http.ResponseWriter, r *http.Request) {
	response, err := h.source.Current(r.Context())
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	writeJSON(w, http.StatusOK, response)
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(value); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}
