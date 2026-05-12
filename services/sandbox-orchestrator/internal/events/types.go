package events

// SubmissionCreated is the event published by the submission service.
type SubmissionCreated struct {
    ID        string `json:"id"`
    Bucket    string `json:"bucket"`
    ObjectKey string `json:"object_key"`
    Language  string `json:"language"`
}