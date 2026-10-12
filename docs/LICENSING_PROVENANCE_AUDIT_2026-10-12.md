# GOT source-rights and license-provenance audit — 2026-10-12

Status: **INCOMPLETE — rights approval required, no license migration authorized**.
Tracking issue: https://github.com/Memorithm/GOT/issues/3.

This is a source-inspection inventory, not a legal opinion, copyright
assignment, conclusion that public code is freely licensed, or permission to
change historical rights. No new license text is installed by this audit.

## Reproducible GitHub findings

- Repository: https://github.com/Memorithm/GOT, default branch main,
  audited at commit 1da4ebd899f596e8a6bc7cfd34ec7afccbe0e8ad.
- GitHub's default-branch commits endpoint returned 18 commit records,
  dated 2026-09-10 to 2026-10-11. The **recursive trees for each of those
  18 commits** contained no file with canonical basename LICENSE,
  LICENSE.md, LICENSING.md, NOTICE, NOTICE.md or COPYRIGHT.
- This is **not evidence** that every branch, release archive, distributed
  copy, source excerpt, or historical third-party agreement lacks license
  conditions. No release appeared in the inspected release endpoint.
- Git author metadata observed in the 18 default-branch commits includes
  MEMOPERF (GitHub account CHECKUPAUTO) and google-labs-jules[bot].
  The bot-authored contribution is the Rust workspace at commit
  16b0f07383b7ee7a21f51122ca91af0b1fe8ee3f (PR #7).
  Author, committer and bot-account labels **do not prove copyright
  ownership, exclusive rights, assignments or authority to relicense**.
- The Rust lockfile pins 78 package/version entries, including the 10
  internal got-* workspace packages and externally sourced packages.
  The Python requirements.lock lists seven distribution/version entries:
  numpy, pandas, python-dateutil, pytz, scipy, six and tzdata.
  Package-name provenance is not a dependency license audit.
- The tracked SciRust bridge is GOT code that invokes a separate binary.
  This inventory does not claim SciRust source was copied into GOT.
  That conclusion would require an actual code-origin review.

## Target reference, not authorization

SciRust separately publishes PolyForm Noncommercial License 1.0.0 with a
Required Notice and commercial licensing pathway:
https://github.com/Memorithm/SciRust/blob/main/LICENSE.md
https://github.com/Memorithm/SciRust/blob/main/LICENSING.md

Those documents do not automatically apply to this repository. In
particular, copying SciRust's copyright holder statement into GOT before
establishing rights for GOT's contributions would be misleading.

## Evidence still missing

1. Written and reviewable contributor-rights inventory for original
   MEMOPERF contributions and other first-party work; verify ownership
   or authorization instead of inferring it from Git login.
2. Written terms and any relevant service/work-product arrangements for
   Jules-generated and other AI-generated contributions. Identify possible
   imports or training-data-derived material where provenance is uncertain.
3. File-level review of imported/adapted code and third-party notices,
   especially any SciRust-derived algorithms or source excerpts, and any
   historical distributed copies. Do not remove vendor notices.
4. A dependency-license/SBOM inventory with per-package authoritative
   licenses for the selected release, including separately distributed
   Python and Rust dependencies. A Cargo.lock file does not contain
   authoritative SPDX license grants.
5. Review of all historical licenses, external distributions, and whether
   already granted permissions survive prospective licensing changes.
6. An explicit recorded decision from parties competent to license each
   eligible contribution, plus counsel review where ownership is disputed.

## Safe interim policy

- Do not add or replace LICENSE/LICENSING.md by inference.
- Do not change Git history or retroactively alter previous grants.
- Do not publish a blanket assertion that all GOT sources are licensed
  under PolyForm until contribution-by-contribution authority is proven.
- Document license and notice evidence before distribution of a packaged
  release; treat third-party dependencies under their own terms.
- Keep issue #3 open until these blockers are resolved with verifiable
  records. A passing software CI cannot substitute for rights evidence.

## Repeatable inspection

With a trusted local checkout and authorized GitHub read access:

    git log main --format='%H %an <%ae>' 
    git ls-tree -r --name-only main
    git grep -niE 'license|copyright|spdx|required notice' main -- \
      README.md AGENTS.md pyproject.toml Cargo.toml crates got || true

These commands inventory potential statements; they **do not** themselves
establish rights or source lineage.
