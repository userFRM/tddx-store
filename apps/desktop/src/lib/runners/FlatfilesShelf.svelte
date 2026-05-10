<script lang="ts">
  /**
   * Browse shelf for the flatfile API: zip-of-CSV-per-day bulk pulls.
   * Six combinations: stock × {trade, quote, trade_quote} and option ×
   * {trade, quote, trade_quote, open_interest, eod, ohlc}. Click any
   * card → FlatfileRunner modal.
   */
  import { FileArchive, Plus } from "lucide-svelte";
  import { app } from "$lib/stores/app.svelte";

  type FF = {
    title: string;
    sec: "STOCK" | "OPTION";
    req: "TRADE" | "QUOTE" | "TRADE_QUOTE" | "OPEN_INTEREST" | "OHLC" | "EOD";
    desc: string;
  };

  const FLATFILES: FF[] = [
    { title: "Stock TRADE flatfile",       sec: "STOCK",  req: "TRADE",         desc: "Every NMS trade for one day, all symbols" },
    { title: "Stock QUOTE flatfile",       sec: "STOCK",  req: "QUOTE",         desc: "Every NBBO update for one day, all symbols" },
    { title: "Stock TRADE-QUOTE flatfile", sec: "STOCK",  req: "TRADE_QUOTE",   desc: "Trades + paired NBBO for one day, all symbols" },
    { title: "Option TRADE flatfile",      sec: "OPTION", req: "TRADE",         desc: "Every option trade for one day, full chain" },
    { title: "Option QUOTE flatfile",      sec: "OPTION", req: "QUOTE",         desc: "Every NBBO option quote for one day, full chain" },
    { title: "Option TRADE-QUOTE flatfile",sec: "OPTION", req: "TRADE_QUOTE",   desc: "Option trades + paired NBBO, full chain" },
    { title: "Option OI flatfile",         sec: "OPTION", req: "OPEN_INTEREST", desc: "Daily open interest snapshot, full chain" },
    { title: "Option EOD flatfile",        sec: "OPTION", req: "EOD",           desc: "Per-strike EOD bars, full chain" },
  ];

  function open(ff: FF) {
    // Reuse the endpoint runner store with a synthetic registry entry.
    app.endpointRunner = {
      endpoint: {
        name: `flatfile_${ff.sec.toLowerCase()}_${ff.req.toLowerCase()}`,
        description: ff.desc,
        category: ff.sec.toLowerCase(),
        subcategory: "flatfile",
        rest_path: `/v3/${ff.sec.toLowerCase()}/flatfile`,
        returns: "ZipArchive",
        params: [
          { name: "date", description: "Trading day YYYYMMDD", param_type: "Date", required: true },
        ],
      },
      args: {},
      format: "parquet",   // ignored by flatfile path; cosmetic.
      busy: false,
      msg: "",
    };
    app.flatfileRunnerOpen = true;
    app.endpointRunnerOpen = false;
    // Stash the flatfile spec on the runner state.
    (app.endpointRunner as unknown as { _flatfile?: FF })._flatfile = ff;
  }
</script>

<section class="shelf">
  <header>
    <h2 class="title">
      <FileArchive size={16} />
      Flatfiles · bulk-day pulls
    </h2>
    <span class="count text-caption">{FLATFILES.length} bundles</span>
  </header>
  <div class="grid">
    {#each FLATFILES as ff (ff.title)}
      <article class="card" onclick={() => open(ff)} role="button" tabindex="0"
               onkeydown={(e) => e.key === "Enter" && open(ff)}>
        <div class="head">
          <span class="sec-pill">{ff.sec}</span>
          <span class="req-pill">{ff.req.replace("_", " ")}</span>
        </div>
        <h3 class="t">{ff.title}</h3>
        <p class="desc">{ff.desc}</p>
        <div class="foot">
          <span class="hint text-caption">One zip / CSV per request</span>
          <button class="run-btn" onclick={(e) => { e.stopPropagation(); open(ff); }}>
            <Plus size={11} /> Download
          </button>
        </div>
      </article>
    {/each}
  </div>
</section>

<style>
  .shelf { display: flex; flex-direction: column; gap: var(--sp-3); }
  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    border-bottom: 1px solid var(--border);
    padding-bottom: var(--sp-2);
  }
  .title {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .count { color: var(--fg-muted); }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--sp-3);
  }
  .card {
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    gap: var(--sp-2);
    height: 168px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-3) var(--sp-4);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-standard),
                border-color var(--dur-fast) var(--ease-standard),
                transform var(--dur-fast) var(--ease-standard);
    outline: none;
  }
  .card:hover {
    background: var(--surface-2);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }
  .head { display: flex; gap: 6px; }
  .sec-pill, .req-pill {
    font-size: 10px;
    font-weight: var(--weight-semi);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 7px;
    border-radius: var(--r-pill);
  }
  .sec-pill { background: var(--accent-tint); color: var(--accent-hi); }
  .req-pill { background: var(--surface-3); color: var(--fg-muted); }
  .t { font-size: var(--text-body); font-weight: var(--weight-semi); margin: 0; }
  .desc {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    margin: 0;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }
  .hint { color: var(--fg-subtle); }
  .run-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px var(--sp-2);
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: 11px;
    font-weight: var(--weight-medium);
    cursor: pointer;
  }
  .run-btn:hover {
    background: var(--accent-tint);
    color: var(--accent-hi);
    border-color: rgba(124,140,255,0.3);
  }
</style>
