# Rivers

- STATUS: OPEN
- PRIORITY: 80
- TAGS: worldgen terrain water

Generate continuous procedural rivers that carve valleys from highland/mountain sources down to the ocean or lakes:
- Carve smooth riverbeds into `WorldConfig::get_raw_height` using domain-warped ridge/Worley paths or downhill flow graphs between macro-cells.
- Render flowing water ribbon meshes (or localized river surface elevation) along carved channels with pebbles/wet-sand riverbanks.
- Integrate with `TownManager` so roads naturally cross rivers over wooden and stone bridges.
