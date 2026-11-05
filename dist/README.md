# Candidate artifact handoff

Binary ZIPs and downloaded CI archives are local/CI artifacts and are not committed to Git. The tracked [artifact manifest](artifact-manifest.json) records exact candidate source, four targets, hashes and verification evidence. See the [release checklist](../docs/release-checklist.md). CI artifacts expire after 14 days; Apple Silicon packaging/smoke was local. Public release publication is a separate authorized action.
