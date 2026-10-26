# Dados dos heróis e dos itens

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

## Itens

`items.json` é um recorte de `build/items.json`, do mesmo commit acima. Ele é a fonte dos nomes de exibição, dos custos e das receitas dos itens.

- Só entram itens com `dname`. As receitas (`recipe_*`) e outros registros sem nome de exibição ficam de fora.
- Campos por item:
  - `id`: o ID numérico da origem, usado pelo OpenDota no inventário e na popularidade de itens;
  - `name`: a chave da fonte com o prefixo `item_`, que é o nome usado pelo GSI (ex.: `satanic` vira `item_satanic`);
  - `display_name`: o `dname`;
  - `cost`: o custo em ouro, ou `null` quando a origem não o informa (alguns itens, como os tokens de neutros);
  - `components`: os componentes da receita, com o mesmo prefixo. Na origem, algumas receitas têm um componente vazio (`""`), que fica de fora.
  - `img`: o caminho da imagem do item no CDN da Steam, como na origem (ex.: `/apps/dota2/images/dota_react/items/blink.png?t=1593393829403`). O aplicativo baixa a imagem em tempo de execução para o cache do usuário; as imagens não são versionadas.
- Um item por linha, na ordem do `id` da origem.

Comando usado para gerar o recorte a partir do arquivo de origem:

```sh
jq -c '[to_entries[] | select(.value.dname != null) | {id: .value.id, name: ("item_" + .key), display_name: .value.dname, cost: .value.cost, components: [(.value.components // [])[] | select(. != null and . != "") | "item_" + .], img: .value.img}] | sort_by(.id) | .[]' items.json \
  | awk 'BEGIN{print "["} NR>1{print prev","} {prev=$0} END{print prev; print "]"}' > assets/dotaconstants/items.json
```

## IDs das habilidades e patches

`ability_ids.json` e `patch.json` são cópias sem alteração de `build/ability_ids.json` e `build/patch.json`, do mesmo commit acima.

- `ability_ids.json`: objeto com o ID da habilidade (em texto) como chave e o nome interno como valor. Uma chave pode ter mais de um ID separado por vírgula (`"3060,1617"`), com o mesmo nome. Traduz os IDs da ordem de habilidades das partidas do OpenDota.
- `patch.json`: lista de patches com `name` (como `7.41`), `date` e `id`. O `id` é o valor do campo `patch` das partidas do OpenDota; o patch atual é o de maior `id`.
