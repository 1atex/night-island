# Contributing / Участие в разработке

[English](#english) · [Русский](#русский)

## English

1. Open an issue first for anything bigger than a small fix, so we agree on the approach.
2. Fork, create a branch from `dev`, keep the change focused on one thing.
3. Before opening a pull request make sure these pass:

```
cargo fmt --check
cargo clippy --all-targets
cargo test
```

4. Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) in English: `feat:`, `fix:`, `refactor:`, `chore:`.
5. Open the pull request against `dev` and fill in the template.

Code guidelines:

- Reuse existing code and well-known crates before writing new code.
- Comments are fine, feel free to add them; the maintainer may strip them later.

## Русский

1. Для всего крупнее мелкой правки сначала откройте issue, чтобы согласовать подход.
2. Сделайте форк, создайте ветку от `dev`, держите изменение сфокусированным на одной задаче.
3. Перед pull request убедитесь, что проходят:

```
cargo fmt --check
cargo clippy --all-targets
cargo test
```

4. Сообщения коммитов по [Conventional Commits](https://www.conventionalcommits.org/) на английском: `feat:`, `fix:`, `refactor:`, `chore:`.
5. Откройте pull request в `dev` и заполните шаблон.

Требования к коду:

- Сначала переиспользуйте существующий код и известные крейты, потом пишите новое.
- Комментарии можно добавлять, если хотите; мейнтейнер может убрать их позже.
