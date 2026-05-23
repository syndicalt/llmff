# apt Repository Design

`llmff` does not currently publish an apt repository. Direct `.deb` downloads
from GitHub Releases remain the supported Debian and Ubuntu installation path.
There are no apt repository installation instructions today. Do not add apt
repository installation instructions until this design is implemented and
reviewed.

## Publication Gate

An apt repository becomes supportable only after maintainers approve all of the
following:

- signed repository metadata for every published suite and architecture;
- both inline `InRelease` metadata and detached `Release.gpg` signatures;
- a documented signing-key custody model;
- key rotation and revocation steps;
- hosting with HTTPS, immutable release retention, and rollback controls;
- historical retention for package versions referenced by existing repository
  metadata;
- recovery steps for stale metadata, missing packages, compromised keys, or
  broken repository clients;
- a post-publication verifier that fetches the repository metadata and confirms
  signature validity before announcement.

## Metadata Shape

The repository metadata must include:

- `Release`: suite, component, architecture, checksum, and size metadata;
- `InRelease`: clear-signed `Release` metadata for default apt clients;
- `Release.gpg`: detached signature for clients that require split metadata;
- `Packages`: binary package index for each supported architecture;
- checksums that match the `.deb` assets published through GitHub Releases.

Unsigned repository metadata must not be checked in or uploaded. If repository
metadata is generated in CI, the job must fail closed when signing material is
missing.

## Operational Ownership

Maintainers must decide who owns:

- signing-key creation, storage, rotation, revocation, and recovery;
- repository hosting, retention, CDN/cache invalidation, and incident response;
- package rollback and superseded package retention;
- user reports when apt clients reject metadata or package signatures.

Until that ownership exists, keep apt parked and keep `packaging/apt` free of
repository metadata.
