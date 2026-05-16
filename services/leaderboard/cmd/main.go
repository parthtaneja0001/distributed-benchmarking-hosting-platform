package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/config"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/httpapi"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/leaderboard"
	redisstore "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/redis"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/scoring"
	wsstream "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/ws"
)

func main() {
	cfg := config.Load()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	store := redisstore.NewStore(cfg.RedisAddr, cfg.RedisKeyPattern)
	leaderboardService := leaderboard.NewService(store, scoring.ProvisionalScorer{})
	wsHandler := wsstream.NewHandler(leaderboardService, cfg.StreamPeriod())
	handler := httpapi.NewHandler(leaderboardService, wsHandler)

	server := &http.Server{
		Addr:              cfg.HTTPAddr,
		Handler:           handler.Routes(),
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		log.Printf("Leaderboard service listening on %s", cfg.HTTPAddr)
		log.Printf("Reading Redis addr=%s pattern=%s", cfg.RedisAddr, cfg.RedisKeyPattern)
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("leaderboard server failed: %v", err)
		}
	}()

	<-ctx.Done()
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := server.Shutdown(shutdownCtx); err != nil {
		log.Printf("leaderboard server shutdown failed: %v", err)
	}
}
