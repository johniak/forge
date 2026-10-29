# Forge — pakiet startowy dla Claude Code

To **pakiet specyfikacyjny**, nie działająca jeszcze implementacja.

## Pliki

- `SPEC.md` — pełna specyfikacja produktu i architektury, wszystkie dotychczasowe ustalenia; w języku angielskim, żeby można było użyć bezpośrednio z agentem.
- `CLAUDE.md` — instrukcje dla Claude Code: jakość, workflow, priorytety, jak raportować wykonane testy i czego nie wolno pomijać.
- `START_PROMPT.md` — gotowy prompt na pierwszą sesję.

## Użycie w nowym repozytorium

1. Utwórz puste repo, np. `forge`, i skopiuj do niego `SPEC.md` i `CLAUDE.md`.
2. Uruchom Claude Code w katalogu tego repozytorium.
3. Wklej zawartość `START_PROMPT.md` jako pierwsze zadanie.
4. Oczekuj małych, działających etapów z rzeczywistymi wynikami testów, a nie implementowania całego frameworka jednym ogromnym commitem.
5. Wymagaj utrzymywania `docs/requirements-matrix.md`, aby żaden temat ze specyfikacji nie zniknął.

## Najważniejsze rozstrzygnięcia

- Backend: Rust, Axum/Tokio/Tower, PostgreSQL-first.
- Modele i migracje: tak; SQLx + bezpieczny dynamiczny query builder zamiast wielkiego ORM.
- Django-like: admin, instalowalne apki, middleware, auth, forms, zadania i CLI.
- Frontend: szybkość użytkownika jako kryterium; wybór React Compiler/Solid/Svelte/Leptos/Qwik wymaga benchmarku; opcjonalny autorski silnik dopiero później.
- Testy: 100% mierzalnego pokrycia linii i gałęzi kodu aplikacji jako wymuszony gate, plus integracja, E2E, migracje, security, mutation/property i testy screenshotów.
- Bezpieczeństwo: bezpieczny SQL/allowlist, walidacja, polityki dostępu, ochrony webowe, sprawdzane pluginy; brak nieuczciwej obietnicy absolutnego 100% bezpieczeństwa.
- Tooling: wspólny silnik CLI+MCP, context engine, graf zależności, impact analysis, test/visual inspector i raport release.

**Ważne:** `SPEC.md` oznacza nierozstrzygnięte kwestie jako OPEN. Nie należy ich "zamykać" zgadywaniem przez Claude.
