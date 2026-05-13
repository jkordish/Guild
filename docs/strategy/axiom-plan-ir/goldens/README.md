# Axiom Plan IR Goldens

These files are checked by:

```bash
cargo run -q -p xtask -- axiom-plan check-goldens
```

They pin docs-local Axiom Plan IR preview and diagnostic output. The preview
goldens are pre-admission review artifacts only: plans are not admitted, not
granted, not executed, have no Guild resolution, and do not create receipts or
persist evidence.

Use update mode only for intentional output changes:

```bash
cargo run -q -p xtask -- axiom-plan check-goldens --update
```

Update mode is scoped to this `goldens/` directory. Review refreshed files as
product-surface changes, not disposable snapshots.
