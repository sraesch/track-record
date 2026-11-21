# ADR 0001: Technology stack

**Status:** accepted · **Date:** 2026-11-21

## Context

The requirements fix Kubernetes + Helm, a relational database, an OpenAPI REST
API, an MCP server and a web UI, but not languages or tooling. The hard parts
are query-time computation over many series (NFR-5, NFR-6) and a derived metric
DSL that is safe and identical in UI, API and MCP (FR-17a).

## Decision

| Area | Choice |
|---|---|
| Database | PostgreSQL, tested with pgTAP |
| Backend | Rust for all services (REST API, MCP server, jobs), tested with `cargo test` |
| Frontend | TypeScript (strict), React, Vite, tested with Vitest |
| i18n | react-i18next |

## Why

- **PostgreSQL:** fits the relational, versioned model; `jsonb` keeps parameter
  types; constraints and triggers can enforce immutability (FR-6). pgTAP tests
  those guarantees directly.
- **Rust:** fast, predictable series processing and a natural fit for a safe
  DSL. One language lets API and MCP share the same query layer.
- **TypeScript + React + Vite + Vitest:** mainstream stack for a data-heavy
  UI; Vitest reuses the Vite config.
- **react-i18next from day one:** retrofitting translations is expensive.

## Consequences

- No hard-coded user-facing strings in components.
- Frontend API types are generated from the OpenAPI spec.
- Development and CI need both a Rust and a Node.js toolchain.

## Left for later ADRs

Rust crates (HTTP, DB access/migrations, MCP SDK) · DSL grammar and how the UI
validates it (API call or Rust compiled to WebAssembly) · cache strategy ·
charting library · repo layout, package manager, CI · UI languages.
Storage is decided in [ADR 0002](0002-series-storage.md) and
[ADR 0003](0003-attachment-storage.md).
