-- Schema v2 is initialized from scratch; older databases are rejected, not silently upgraded.
CREATE TABLE analysis_artifacts(
 generation_id INTEGER PRIMARY KEY REFERENCES generations(id), record_json TEXT NOT NULL
) STRICT;
