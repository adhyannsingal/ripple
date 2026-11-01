CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL) STRICT;
CREATE TABLE generations(
 id INTEGER PRIMARY KEY CHECK(id>0), metadata_json TEXT NOT NULL, seal TEXT NOT NULL,
 complete INTEGER NOT NULL DEFAULT 0 CHECK(complete IN (0,1))
) STRICT;
CREATE TABLE captured_inputs(
 generation_id INTEGER NOT NULL REFERENCES generations(id), path TEXT NOT NULL,
 role TEXT NOT NULL CHECK(role IN ('source','configuration','ignore_policy','package_manifest','lockfile')),
 byte_len INTEGER NOT NULL CHECK(byte_len>=0), hash_version INTEGER NOT NULL CHECK(hash_version=1),
 digest TEXT NOT NULL CHECK(length(digest)=64 AND digest NOT GLOB '*[^0-9a-f]*'), bytes BLOB NOT NULL,
 PRIMARY KEY(generation_id,path), CHECK(length(bytes)=byte_len)
) STRICT;
CREATE TABLE nodes(
 generation_id INTEGER NOT NULL REFERENCES generations(id), node_id TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('FILE','FUNCTION','CLASS','METHOD','TEST_FILE','OPAQUE_BOUNDARY')),
 record_json TEXT NOT NULL, PRIMARY KEY(generation_id,node_id)
) STRICT;
CREATE TABLE files(
 generation_id INTEGER NOT NULL, path TEXT NOT NULL, node_id TEXT NOT NULL, facts_json TEXT NOT NULL,
 PRIMARY KEY(generation_id,path), UNIQUE(generation_id,node_id),
 FOREIGN KEY(generation_id,node_id) REFERENCES nodes(generation_id,node_id),
 FOREIGN KEY(generation_id,path) REFERENCES captured_inputs(generation_id,path)
) STRICT;
CREATE TABLE symbols(
 generation_id INTEGER NOT NULL, node_id TEXT NOT NULL, file_path TEXT NOT NULL, record_json TEXT NOT NULL,
 PRIMARY KEY(generation_id,node_id),
 FOREIGN KEY(generation_id,node_id) REFERENCES nodes(generation_id,node_id),
 FOREIGN KEY(generation_id,file_path) REFERENCES files(generation_id,path)
) STRICT;
CREATE TABLE edges(
 generation_id INTEGER NOT NULL, ordinal INTEGER NOT NULL CHECK(ordinal>=0), source_node_id TEXT NOT NULL,
 target_node_id TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('DEFINES','IMPORTS','CALLS','REFERENCES','REEXPORTS')),
 confidence TEXT NOT NULL CHECK(confidence IN ('RESOLVED','CANDIDATE')), record_json TEXT NOT NULL,
 PRIMARY KEY(generation_id,ordinal),
 FOREIGN KEY(generation_id,source_node_id) REFERENCES nodes(generation_id,node_id),
 FOREIGN KEY(generation_id,target_node_id) REFERENCES nodes(generation_id,node_id)
) STRICT;
CREATE TABLE unresolved_references(
 generation_id INTEGER NOT NULL, ordinal INTEGER NOT NULL CHECK(ordinal>=0), source_node_id TEXT NOT NULL,
 boundary_node_id TEXT NOT NULL, record_json TEXT NOT NULL,
 PRIMARY KEY(generation_id,ordinal), UNIQUE(generation_id,boundary_node_id),
 FOREIGN KEY(generation_id,source_node_id) REFERENCES nodes(generation_id,node_id),
 FOREIGN KEY(generation_id,boundary_node_id) REFERENCES nodes(generation_id,node_id)
) STRICT;
CREATE TABLE current_generation(
 singleton INTEGER PRIMARY KEY CHECK(singleton=1), generation_id INTEGER NOT NULL REFERENCES generations(id)
) STRICT;
CREATE TRIGGER current_requires_complete_insert BEFORE INSERT ON current_generation
 WHEN COALESCE((SELECT complete FROM generations WHERE id=NEW.generation_id),0)<>1
 BEGIN SELECT RAISE(ABORT,'generation is not complete'); END;
CREATE TRIGGER current_requires_complete_update BEFORE UPDATE ON current_generation
 WHEN COALESCE((SELECT complete FROM generations WHERE id=NEW.generation_id),0)<>1
 BEGIN SELECT RAISE(ABORT,'generation is not complete'); END;
CREATE INDEX edges_by_target ON edges(generation_id,target_node_id);
CREATE INDEX edges_by_source ON edges(generation_id,source_node_id);
