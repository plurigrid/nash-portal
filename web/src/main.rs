use std::{cell::RefCell, io, rc::Rc};

use gloo_timers::{callback::Interval, future::TimeoutFuture};
use js_sys::Math;
use ratzilla::ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame, Terminal,
};
use ratzilla::{event::KeyCode, DomBackend, WebRenderer};
use wasm_bindgen_futures::spawn_local;

const NASH: &str = "4DQsMSkeKc3Mcij1BE4Z8oqU3QeV45QQ3Psn3CNDpump";
const POOL: &str = "FwuB9juwaoo35C2Nx6XMVn3sQ6B9HeXuuquR7rB21y3Y";

#[derive(Default, Clone)]
struct TokenData {
    price_usd: f64,
    price_sol: f64,
    market_cap: f64,
    fdv: f64,
    volume_24h: f64,
    change_5m: f64,
    change_1h: f64,
    change_24h: f64,
    buys_24h: u64,
    sells_24h: u64,
    liquidity: f64,
    txns: u64,
}

#[derive(Clone)]
struct Candle {
    o: f64,
    h: f64,
    l: f64,
    c: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Timeframe {
    Min1,
    Min5,
    Min15,
    Hour1,
    Day1,
}

impl Timeframe {
    fn label(&self) -> &str {
        match self {
            Self::Min1 => "1m",
            Self::Min5 => "5m",
            Self::Min15 => "15m",
            Self::Hour1 => "1H",
            Self::Day1 => "1D",
        }
    }

    fn api_params(&self) -> (&str, u32, u32) {
        match self {
            Self::Min1 => ("minute", 1, 200),
            Self::Min5 => ("minute", 5, 200),
            Self::Min15 => ("minute", 15, 200),
            Self::Hour1 => ("hour", 1, 100),
            Self::Day1 => ("day", 1, 100),
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::Min1 => Self::Min5,
            Self::Min5 => Self::Min15,
            Self::Min15 => Self::Hour1,
            Self::Hour1 => Self::Day1,
            Self::Day1 => Self::Min1,
        }
    }
}

struct App {
    data: TokenData,
    candles: Vec<Candle>,
    timeframe: Timeframe,
    ticker_offset: usize,
    tick: u64,
    status: String,
}

impl App {
    fn new() -> Self {
        Self {
            data: TokenData::default(),
            candles: Vec::new(),
            timeframe: Timeframe::Min1,
            ticker_offset: 0,
            tick: 0,
            status: "Loading...".into(),
        }
    }

    fn ticker_text(&self) -> String {
        let d = &self.data;
        let arrow = |v: f64| {
            if v > 0.0 {
                "▲"
            } else if v < 0.0 {
                "▼"
            } else {
                "─"
            }
        };
        format!(
            "    NASH ${:.8}  {}5m {:+.1}%  {}1h {:+.1}%  {}24h {:+.1}%  │  MCap ${:.0}K  Vol ${:.0}K  │  B:{} S:{}  │  {:.6} SOL  ◆  pump.fun    ",
            d.price_usd,
            arrow(d.change_5m),
            d.change_5m,
            arrow(d.change_1h),
            d.change_1h,
            arrow(d.change_24h),
            d.change_24h,
            d.market_cap / 1000.0,
            d.volume_24h / 1000.0,
            d.buys_24h,
            d.sells_24h,
            d.price_sol
        )
    }
}

fn parse_dexscreener(v: &serde_json::Value) -> Result<TokenData, &'static str> {
    let p = v["pairs"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or("no pairs")?;
    let buys = p["txns"]["h24"]["buys"].as_u64().unwrap_or(0);
    let sells = p["txns"]["h24"]["sells"].as_u64().unwrap_or(0);
    Ok(TokenData {
        price_usd: p["priceUsd"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        price_sol: p["priceNative"]
            .as_str()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0.0),
        market_cap: p["marketCap"].as_f64().unwrap_or(0.0),
        fdv: p["fdv"].as_f64().unwrap_or(0.0),
        volume_24h: p["volume"]["h24"].as_f64().unwrap_or(0.0),
        change_5m: p["priceChange"]["m5"].as_f64().unwrap_or(0.0),
        change_1h: p["priceChange"]["h1"].as_f64().unwrap_or(0.0),
        change_24h: p["priceChange"]["h24"].as_f64().unwrap_or(0.0),
        buys_24h: buys,
        sells_24h: sells,
        liquidity: p["liquidity"]["usd"].as_f64().unwrap_or(0.0),
        txns: buys + sells,
    })
}

fn parse_jupiter_price(v: &serde_json::Value, mint: &str) -> Result<f64, &'static str> {
    let s = v["data"][mint]["price"]
        .as_str()
        .ok_or("no jupiter price")?;
    s.parse().map_err(|_| "jupiter parse")
}

fn parse_geckoterminal(v: &serde_json::Value) -> Result<Vec<Candle>, &'static str> {
    let list = v["data"]["attributes"]["ohlcv_list"]
        .as_array()
        .ok_or("no ohlcv")?;
    let mut candles: Vec<Candle> = list
        .iter()
        .filter_map(|c| {
            let a = c.as_array()?;
            Some(Candle {
                o: a.get(1)?.as_f64()?,
                h: a.get(2)?.as_f64()?,
                l: a.get(3)?.as_f64()?,
                c: a.get(4)?.as_f64()?,
            })
        })
        .collect();
    candles.reverse();
    Ok(candles)
}

async fn fetch_dexscreener() -> Option<TokenData> {
    let url = format!("https://api.dexscreener.com/latest/dex/tokens/{}", NASH);
    let resp = gloo_net::http::Request::get(&url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    parse_dexscreener(&v).ok()
}

async fn fetch_jupiter_price() -> Option<f64> {
    let url = format!("https://api.jup.ag/price/v2?ids={}", NASH);
    let resp = gloo_net::http::Request::get(&url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    parse_jupiter_price(&v, NASH).ok()
}

fn period_millis(tf: Timeframe) -> u32 {
    match tf {
        Timeframe::Min1 => 60_000,
        Timeframe::Min5 => 300_000,
        Timeframe::Min15 => 900_000,
        Timeframe::Hour1 => 3_600_000,
        Timeframe::Day1 => 86_400_000,
    }
}

fn random_millis(max_exclusive: u32) -> u32 {
    if max_exclusive == 0 {
        0
    } else {
        (Math::random() * f64::from(max_exclusive)).floor() as u32
    }
}

fn jitter_millis(base: u32) -> u32 {
    let extra = (base / 4).max(1);
    base.saturating_add(random_millis(extra.saturating_add(1)))
}

async fn fetch_candles(tf: Timeframe) -> Option<Vec<Candle>> {
    let (period, agg, limit) = tf.api_params();
    let url = format!(
        "https://api.geckoterminal.com/api/v2/networks/solana/pools/{}/ohlcv/{}?aggregate={}&limit={}&currency=usd",
        POOL, period, agg, limit
    );
    let resp = gloo_net::http::Request::get(&url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    parse_geckoterminal(&v).ok()
}

fn merge_fast_price(mut aux: TokenData, fast_px: f64) -> TokenData {
    if fast_px > 0.0 {
        aux.price_usd = fast_px;
    }
    aux
}

fn epsilon_greedy_jupiter(roll: f32) -> bool {
    roll < 0.9
}

fn do_refresh(state: &Rc<RefCell<App>>) {
    let state = Rc::clone(state);
    spawn_local(async move {
        let fast_px = { state.borrow().data.price_usd };
        if let Some(data) = fetch_dexscreener().await {
            state.borrow_mut().data = merge_fast_price(data, fast_px);
        }

        let tf = { state.borrow().timeframe };
        if let Some(candles) = fetch_candles(tf).await {
            let mut app = state.borrow_mut();
            app.candles = candles;
            app.tick += 1;
            app.status = format!("{} candles │ live", app.candles.len());
        }
    });
}

fn spawn_fast_price_loop(state: &Rc<RefCell<App>>) {
    let state = Rc::clone(state);
    spawn_local(async move {
        loop {
            if epsilon_greedy_jupiter(Math::random() as f32) {
                if let Some(px) = fetch_jupiter_price().await {
                    let mut app = state.borrow_mut();
                    app.data.price_usd = px;
                    app.status = format!("jup ${:.8}", px);
                }
            } else if let Some(data) = fetch_dexscreener().await {
                let mut app = state.borrow_mut();
                app.data = data;
                app.status = "dex (probe)".into();
            }

            TimeoutFuture::new(5_000 + random_millis(2_001)).await;
        }
    });
}

fn spawn_aux_loop(state: &Rc<RefCell<App>>) {
    let state = Rc::clone(state);
    spawn_local(async move {
        loop {
            if let Some(data) = fetch_dexscreener().await {
                let fast_px = { state.borrow().data.price_usd };
                state.borrow_mut().data = merge_fast_price(data, fast_px);
            }

            TimeoutFuture::new(30_000 + random_millis(10_001)).await;
        }
    });
}

fn spawn_candle_loop(state: &Rc<RefCell<App>>) {
    let state = Rc::clone(state);
    spawn_local(async move {
        loop {
            let tf = { state.borrow().timeframe };
            if let Some(candles) = fetch_candles(tf).await {
                let mut app = state.borrow_mut();
                app.candles = candles;
                app.tick += 1;
            }

            let last_tf = tf;
            let total = jitter_millis(period_millis(tf));
            let mut elapsed = 0u32;

            while elapsed < total {
                TimeoutFuture::new(1_000).await;
                elapsed = elapsed.saturating_add(1_000);
                if state.borrow().timeframe != last_tf {
                    break;
                }
            }
        }
    });
}

fn render_candles(f: &mut Frame, area: Rect, candles: &[Candle], tf: Timeframe) {
    if candles.is_empty() {
        f.render_widget(
            Paragraph::new("  Loading candles from GeckoTerminal...")
                .block(Block::default().borders(Borders::ALL).title(" Candles ")),
            area,
        );
        return;
    }
    let w = area.width.saturating_sub(2) as usize;
    let h = area.height.saturating_sub(2) as usize;
    if h < 3 || w < 5 {
        return;
    }

    let max_candles = w / 2;
    let visible = if candles.len() > max_candles {
        &candles[candles.len() - max_candles..]
    } else {
        candles
    };

    let all_high = visible.iter().map(|c| c.h).fold(0.0_f64, f64::max);
    let all_low = visible.iter().map(|c| c.l).fold(f64::MAX, f64::min);
    let range = if (all_high - all_low).abs() < 1e-15 {
        1e-10
    } else {
        all_high - all_low
    };

    let norm = |v: f64| -> usize {
        let pct = (v - all_low) / range;
        ((1.0 - pct) * (h as f64 - 1.0))
            .round()
            .max(0.0)
            .min((h - 1) as f64) as usize
    };

    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', Color::DarkGray); w]; h];

    for (i, candle) in visible.iter().enumerate() {
        let col = i * 2;
        if col >= w {
            break;
        }
        let bullish = candle.c >= candle.o;
        let color = if bullish { Color::Green } else { Color::Red };
        let hi = norm(candle.h);
        let lo = norm(candle.l);
        let otop = norm(candle.o.max(candle.c));
        let obot = norm(candle.o.min(candle.c));
        for row in hi..otop {
            if row < h {
                grid[row][col] = ('│', color);
            }
        }
        for row in otop..=obot {
            if row < h {
                grid[row][col] = ('┃', color);
                if col + 1 < w {
                    grid[row][col + 1] = ('┃', color);
                }
            }
        }
        for row in (obot + 1)..=lo {
            if row < h {
                grid[row][col] = ('│', color);
            }
        }
    }

    if w > 12 {
        let hi_s = format!("{:.8}", all_high);
        let lo_s = format!("{:.8}", all_low);
        let mid_s = format!("{:.8}", (all_high + all_low) / 2.0);
        for (ci, ch) in hi_s.chars().enumerate() {
            if w.saturating_sub(11) + ci < w {
                grid[0][w - 11 + ci] = (ch, Color::Yellow);
            }
        }
        if h > 1 {
            for (ci, ch) in lo_s.chars().enumerate() {
                if w.saturating_sub(11) + ci < w {
                    grid[h - 1][w - 11 + ci] = (ch, Color::Yellow);
                }
            }
        }
        if h > 4 {
            for (ci, ch) in mid_s.chars().enumerate() {
                if w.saturating_sub(11) + ci < w {
                    grid[h / 2][w - 11 + ci] = (ch, Color::DarkGray);
                }
            }
        }
    }

    let lines: Vec<Line> = grid
        .iter()
        .map(|row| {
            Line::from(
                row.iter()
                    .map(|(ch, color)| Span::styled(ch.to_string(), Style::default().fg(*color)))
                    .collect::<Vec<Span>>(),
            )
        })
        .collect();

    let last = visible.last().unwrap();
    let pct = if range > 0.0 {
        ((last.c - all_low) / range * 100.0) as i32
    } else {
        50
    };
    let title = format!(
        " {} │ {} candles │ ${:.8} │ {}% ",
        tf.label(),
        visible.len(),
        last.c,
        pct
    );

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(12),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    let txt = app.ticker_text();
    let chars: Vec<char> = txt.chars().collect();
    let w = outer[0].width as usize;
    if !chars.is_empty() {
        let off = app.ticker_offset % chars.len();
        let vis: String = (0..w).map(|i| chars[(off + i) % chars.len()]).collect();
        let tc = if app.data.change_5m > 0.0 {
            Color::Green
        } else if app.data.change_5m < 0.0 {
            Color::Red
        } else {
            Color::Yellow
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                vis,
                Style::default().fg(tc).add_modifier(Modifier::BOLD),
            )),
            outer[0],
        );
    }

    let d = &app.data;
    let pc = if d.change_5m > 0.0 {
        Color::Green
    } else if d.change_5m < 0.0 {
        Color::Red
    } else {
        Color::White
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " NASH ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ${:.8}", d.price_usd),
                Style::default().fg(pc).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {:.6} SOL", d.price_sol),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!(
                    "  MCap ${:.0}K  FDV ${:.0}K",
                    d.market_cap / 1000.0,
                    d.fdv / 1000.0
                ),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("  │ {}", app.status),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        outer[1],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(outer[2]);
    let cf = |v: f64| {
        if v > 0.0 {
            Color::Green
        } else if v < 0.0 {
            Color::Red
        } else {
            Color::Gray
        }
    };

    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" 5m "),
                Span::styled(
                    format!("{:+.2}%", d.change_5m),
                    Style::default()
                        .fg(cf(d.change_5m))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw(" 1h "),
                Span::styled(
                    format!("{:+.2}%", d.change_1h),
                    Style::default().fg(cf(d.change_1h)),
                ),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Δ │ 24h {:+.1}% ", d.change_24h))
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        cols[0],
    );

    f.render_widget(
        Paragraph::new(vec![
            Line::from(format!(" Vol ${:.0}", d.volume_24h)),
            Line::from(format!(" Liq ${:.0}", d.liquidity)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Volume ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        cols[1],
    );

    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" B:"),
                Span::styled(format!("{}", d.buys_24h), Style::default().fg(Color::Green)),
                Span::raw(" S:"),
                Span::styled(format!("{}", d.sells_24h), Style::default().fg(Color::Red)),
            ]),
            Line::from(format!(" Tot {}", d.txns)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Trades ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        cols[2],
    );

    render_candles(f, outer[3], &app.candles, app.timeframe);

    let total = (d.buys_24h + d.sells_24h) as f64;
    let bp = if total > 0.0 {
        d.buys_24h as f64 / total
    } else {
        0.5
    };
    let gc = if bp > 0.55 {
        Color::Green
    } else if bp < 0.45 {
        Color::Red
    } else {
        Color::Yellow
    };
    f.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Buy Pressure ")
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .gauge_style(Style::default().fg(gc).bg(Color::DarkGray))
            .ratio(bp)
            .label(format!("{}% buy", (bp * 100.0) as u32)),
        outer[4],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" t", Style::default().fg(Color::Yellow)),
            Span::raw(":timeframe "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(":refresh "),
            Span::raw(format!(
                " │ [{}] │ tick {} │ GeckoTerminal OHLCV + Jupiter probe",
                app.timeframe.label(),
                app.tick
            )),
        ]))
        .style(Style::default().fg(Color::DarkGray)),
        outer[5],
    );
}

fn main() -> io::Result<()> {
    let state = Rc::new(RefCell::new(App::new()));

    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    let key_state = Rc::clone(&state);
    terminal.on_key_event(move |key_event| match key_event.code {
        KeyCode::Char('t') => {
            {
                let mut app = key_state.borrow_mut();
                app.timeframe = app.timeframe.next();
                app.candles.clear();
                app.status = format!("Switching to {}...", app.timeframe.label());
            }
            do_refresh(&key_state);
        }
        KeyCode::Char('r') => {
            {
                let mut app = key_state.borrow_mut();
                app.candles.clear();
                app.status = "Refreshing...".into();
            }
            do_refresh(&key_state);
        }
        _ => {}
    });

    let ticker_state = Rc::clone(&state);
    let ticker = Interval::new(150, move || {
        ticker_state.borrow_mut().ticker_offset += 1;
    });

    spawn_fast_price_loop(&state);
    spawn_aux_loop(&state);
    spawn_candle_loop(&state);
    do_refresh(&state);

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |f| {
        let app = render_state.borrow();
        draw(f, &app);
    });

    std::mem::forget(ticker);

    Ok(())
}
