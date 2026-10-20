ghostrefs находит символы, единственными известными потребителями которых являются тесты.

Сейчас реализован сбор символов через LSP для Python. Для запуска нужен
`pyright-langserver` в `PATH`.

```sh
cargo run -- --root . --source fixtures/python/project --tests fixtures/python/tests
```

Сканер рекурсивно находит Python-файлы, затем LSP открывает их и возвращает
объявления через `textDocument/documentSymbol`. Результат пока выводится в консоль.
Настройки сервера находятся в `ghostrefs-lsp.json`. Сбор ссылок и анализ графа
пока не реализованы.
