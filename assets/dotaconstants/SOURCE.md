# Dados dos heróis

`heroes.json` é um recorte dos arquivos públicos do [odota/dotaconstants](https://github.com/odota/dotaconstants), licenciados sob MIT (ver `LICENSE` nesta pasta). Ele fica embutido no binário e é a fonte dos dados estáticos da aba Herói.

- Commit: `5c0242df9057848fa2036ca3fafe9f2bd4508163` (2026-10-12).
- Arquivos de origem: `build/heroes.json`, `build/hero_abilities.json` e `build/abilities.json`.
- Campos mantidos por herói:
  - de `heroes.json`: `id`, `name`, `localized_name`, `primary_attr`, `attack_type` e `roles`;
  - de `hero_abilities.json`: as habilidades, sem `generic_hidden`;
  - de `abilities.json`: o `dname` de cada habilidade, como `display_name`. Habilidades sem `dname` ficam de fora.
- Um herói por linha, ordenados por `id`.
- Os textos estão em inglês, como na origem. Os nomes de exibição não foram corrigidos: alguns vêm assim da fonte (ex.: `APPLICATION DAMAGE:` no Batrider).

A origem não tem dificuldade nem posição (1 a 5) dos heróis, e esses dados não são deduzidos aqui.

Para atualizar, gere o recorte a partir de um commit novo com o mesmo filtro e registre o commit e a data acima.
