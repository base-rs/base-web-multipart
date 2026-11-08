# Repository Guidance

Read `readme.md` and the guides in `context/` for the crate's purpose and usage. Before planning or changing behavior, discover and read applicable `design/*.md`, especially `design/scope.md` before adding public items. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.

Install shared guidance with `cargo bake agent:context:install`; discover it with `cargo bake agent:context:list` and `cargo bake agent:context:show`. Keep private task tooling out of the library's dependency graph.

Parser changes must keep `tests/multipart_parser.rs`'s split invariant: a body parsed in any pieces gives the same parts as the whole body.

Use `base-security-audit` for security reviews and record them in `security/audit-*.md`. Use `base-security-advisory` for vulnerability reports and potentially advisory-worthy findings. Read relevant `// SECURITY:` comments and `design/*.md` before reporting a security finding.
