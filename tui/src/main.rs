use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Gauge},
    Frame, Terminal,
};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::Mutex;

const NASH: &str = "4DQsMSkeKc3Mcij1BE4Z8oqU3QeV45QQ3Psn3CNDpump";
const POOL: &str = "FwuB9juwaoo35C2Nx6XMVn3sQ6B9HeXuuquR7rB21y3Y";

#[derive(Default, Clone)]
struct TokenData {
    price_usd: f64, price_sol: f64, market_cap: f64, fdv: f64,
    volume_24h: f64, change_5m: f64, change_1h: f64, change_24h: f64,
    buys_24h: u64, sells_24h: u64, liquidity: f64, txns: u64,
}

#[derive(Clone, Debug)]
struct Candle { ts: i64, o: f64, h: f64, l: f64, c: f64, v: f64 }

#[derive(Clone, Copy, PartialEq)]
enum Timeframe { Min1, Min5, Min15, Hour1, Day1 }
impl Timeframe {
    fn label(&self) -> &str {
        match self { Self::Min1=>"1m", Self::Min5=>"5m", Self::Min15=>"15m", Self::Hour1=>"1H", Self::Day1=>"1D" }
    }
    fn api_params(&self) -> (&str, u32, u32) {
        match self {
            Self::Min1  => ("minute", 1, 200),
            Self::Min5  => ("minute", 5, 200),
            Self::Min15 => ("minute", 15, 200),
            Self::Hour1 => ("hour", 1, 100),
            Self::Day1  => ("day", 1, 100),
        }
    }
    fn next(&self) -> Self {
        match self { Self::Min1=>Self::Min5, Self::Min5=>Self::Min15, Self::Min15=>Self::Hour1, Self::Hour1=>Self::Day1, Self::Day1=>Self::Min1 }
    }
}

struct App {
    data: TokenData,
    candles: Vec<Candle>,
    timeframe: Timeframe,
    ticker_offset: usize,
    tick: u64,
    status: String,
    quit: bool,
}

impl App {
    fn new() -> Self {
        Self { data: TokenData::default(), candles: Vec::new(), timeframe: Timeframe::Min1,
               ticker_offset: 0, tick: 0, status: "Loading...".into(), quit: false }
    }
    fn ticker_text(&self) -> String {
        let d = &self.data;
        let a = |v: f64| if v > 0.0 {"▲"} else if v < 0.0 {"▼"} else {"─"};
        format!("    NASH ${:.8}  {}5m {:+.1}%  {}1h {:+.1}%  {}24h {:+.1}%  │  MCap ${:.0}K  Vol ${:.0}K  │  B:{} S:{}  │  {:.6} SOL  ◆  pump.fun    ",
            d.price_usd, a(d.change_5m), d.change_5m, a(d.change_1h), d.change_1h,
            a(d.change_24h), d.change_24h, d.market_cap/1000.0, d.volume_24h/1000.0,
            d.buys_24h, d.sells_24h, d.price_sol)
    }
}

async fn fetch_price() -> Result<TokenData, Box<dyn std::error::Error + Send + Sync>> {
    let v: serde_json::Value = reqwest::get(&format!("https://api.dexscreener.com/latest/dex/tokens/{}", NASH)).await?.json().await?;
    let p = v["pairs"].as_array().and_then(|a| a.first()).ok_or("no pairs")?;
    Ok(TokenData {
        price_usd: p["priceUsd"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        price_sol: p["priceNative"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        market_cap: p["marketCap"].as_f64().unwrap_or(0.0),
        fdv: p["fdv"].as_f64().unwrap_or(0.0),
        volume_24h: p["volume"]["h24"].as_f64().unwrap_or(0.0),
        change_5m: p["priceChange"]["m5"].as_f64().unwrap_or(0.0),
        change_1h: p["priceChange"]["h1"].as_f64().unwrap_or(0.0),
        change_24h: p["priceChange"]["h24"].as_f64().unwrap_or(0.0),
        buys_24h: p["txns"]["h24"]["buys"].as_u64().unwrap_or(0),
        sells_24h: p["txns"]["h24"]["sells"].as_u64().unwrap_or(0),
        liquidity: p["liquidity"]["usd"].as_f64().unwrap_or(0.0),
        txns: p["txns"]["h24"]["buys"].as_u64().unwrap_or(0) + p["txns"]["h24"]["sells"].as_u64().unwrap_or(0),
    })
}

async fn fetch_candles(tf: Timeframe) -> Result<Vec<Candle>, Box<dyn std::error::Error + Send + Sync>> {
    let (period, agg, limit) = tf.api_params();
    let url = format!("https://api.geckoterminal.com/api/v2/networks/solana/pools/{}/ohlcv/{}?aggregate={}&limit={}&currency=usd", POOL, period, agg, limit);
    let v: serde_json::Value = reqwest::get(&url).await?.json().await?;
    let list = v["data"]["attributes"]["ohlcv_list"].as_array().ok_or("no ohlcv")?;
    let mut candles: Vec<Candle> = list.iter().filter_map(|c| {
        let a = c.as_array()?;
        Some(Candle {
            ts: a.first()?.as_i64()?,
            o: a.get(1)?.as_f64()?, h: a.get(2)?.as_f64()?,
            l: a.get(3)?.as_f64()?, c: a.get(4)?.as_f64()?,
            v: a.get(5)?.as_f64().unwrap_or(0.0),
        })
    }).collect();
    candles.reverse(); // oldest first
    Ok(candles)
}

fn render_candles(f: &mut Frame, area: ratatui::layout::Rect, candles: &[Candle], tf: Timeframe) {
    if candles.is_empty() {
        f.render_widget(Paragraph::new("  Loading candles from GeckoTerminal...")
            .block(Block::default().borders(Borders::ALL).title(" Candles ")), area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let h = area.height.saturating_sub(2) as usize;
    if h < 3 || w < 5 { return; }

    // Use 2 cols per candle (body + gap)
    let max_candles = w / 2;
    let visible = if candles.len() > max_candles { &candles[candles.len()-max_candles..] } else { candles };

    let all_high = visible.iter().map(|c| c.h).fold(0.0_f64, f64::max);
    let all_low = visible.iter().map(|c| c.l).fold(f64::MAX, f64::min);
    let range = if (all_high - all_low).abs() < 1e-15 { 1e-10 } else { all_high - all_low };

    let norm = |v: f64| -> usize {
        let pct = (v - all_low) / range;
        let row = ((1.0 - pct) * (h as f64 - 1.0)).round() as usize;
        row.min(h - 1)
    };

    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', Color::DarkGray); w]; h];

    for (i, candle) in visible.iter().enumerate() {
        let col = i * 2;
        if col >= w { break; }

        let bullish = candle.c >= candle.o;
        let color = if bullish { Color::Green } else { Color::Red };

        let hi = norm(candle.h);
        let lo = norm(candle.l);
        let otop = norm(candle.o.max(candle.c));
        let obot = norm(candle.o.min(candle.c));

        // Upper wick
        for row in hi..otop { if row < h { grid[row][col] = ('│', color); } }
        // Body
        for row in otop..=obot {
            if row < h {
                grid[row][col] = if bullish { ('┃', Color::Green) } else { ('┃', Color::Red) };
                // Fill adjacent col for wider body
                if col + 1 < w {
                    grid[row][col+1] = if bullish { ('┃', Color::Green) } else { ('┃', Color::Red) };
                }
            }
        }
        // Lower wick
        for row in (obot+1)..=lo { if row < h { grid[row][col] = ('│', color); } }
    }

    // Price labels on right edge
    if w > 12 {
        let hi_str = format!("{:.8}", all_high);
        let lo_str = format!("{:.8}", all_low);
        let mid = (all_high + all_low) / 2.0;
        let mid_str = format!("{:.8}", mid);
        for (ci, ch) in hi_str.chars().enumerate() {
            if w - 11 + ci < w { grid[0][w - 11 + ci] = (ch, Color::Yellow); }
        }
        for (ci, ch) in lo_str.chars().enumerate() {
            if w - 11 + ci < w && h > 1 { grid[h-1][w - 11 + ci] = (ch, Color::Yellow); }
        }
        if h > 4 {
            for (ci, ch) in mid_str.chars().enumerate() {
                if w - 11 + ci < w { grid[h/2][w - 11 + ci] = (ch, Color::DarkGray); }
            }
        }
    }

    let lines: Vec<Line> = grid.iter().map(|row| {
        Line::from(row.iter().map(|(ch, color)| Span::styled(ch.to_string(), Style::default().fg(*color))).collect::<Vec<Span>>())
    }).collect();

    let last = visible.last().unwrap();
    let pct_from_low = if range > 0.0 { ((last.c - all_low) / range * 100.0) as i32 } else { 50 };
    let title = format!(" {} │ {} candles │ last ${:.8} │ {}% from low ",
        tf.label(), visible.len(), last.c, pct_from_low);

    f.render_widget(Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title)
        .border_style(Style::default().fg(Color::DarkGray))), area);
}

fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1),   // ticker
        Constraint::Length(3),   // price
        Constraint::Length(5),   // stats
        Constraint::Min(12),    // CANDLES
        Constraint::Length(3),  // gauge
        Constraint::Length(1),   // footer
    ]).split(f.area());

    // Ticker
    let txt = app.ticker_text();
    let chars: Vec<char> = txt.chars().collect();
    let w = outer[0].width as usize;
    let off = app.ticker_offset % chars.len().max(1);
    let vis: String = (0..w).map(|i| chars[(off+i) % chars.len().max(1)]).collect();
    let tc = if app.data.change_5m > 0.0 {Color::Green} else if app.data.change_5m < 0.0 {Color::Red} else {Color::Yellow};
    f.render_widget(Paragraph::new(Span::styled(vis, Style::default().fg(tc).bg(Color::Black).add_modifier(Modifier::BOLD))), outer[0]);

    // Price
    let pc = if app.data.change_5m > 0.0 {Color::Green} else if app.data.change_5m < 0.0 {Color::Red} else {Color::White};
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(" NASH ", Style::default().fg(Color::White).bg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  ${:.8}", app.data.price_usd), Style::default().fg(pc).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {:.6} SOL", app.data.price_sol), Style::default().fg(Color::Cyan)),
        Span::styled(format!("  MCap ${:.0}K  FDV ${:.0}K", app.data.market_cap/1000.0, app.data.fdv/1000.0), Style::default().fg(Color::Yellow)),
        Span::styled(format!("  │ {}", app.status), Style::default().fg(Color::DarkGray)),
    ])).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))), outer[1]);

    // Stats
    let cols = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(34), Constraint::Percentage(33)])
        .split(outer[2]);
    let d = &app.data;
    let cf = |v: f64| if v > 0.0 {Color::Green} else if v < 0.0 {Color::Red} else {Color::Gray};

    f.render_widget(Paragraph::new(vec![
        Line::from(vec![Span::raw(" 5m "), Span::styled(format!("{:+.2}%", d.change_5m), Style::default().fg(cf(d.change_5m)).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::raw(" 1h "), Span::styled(format!("{:+.2}%", d.change_1h), Style::default().fg(cf(d.change_1h)))]),
    ]).block(Block::default().borders(Borders::ALL).title(format!(" Δ │ 24h {:+.1}% ", d.change_24h)).border_style(Style::default().fg(Color::DarkGray))), cols[0]);

    f.render_widget(Paragraph::new(vec![
        Line::from(format!(" Vol ${:.0}", d.volume_24h)),
        Line::from(format!(" Liq ${:.0}", d.liquidity)),
    ]).block(Block::default().borders(Borders::ALL).title(" Volume ").border_style(Style::default().fg(Color::DarkGray))), cols[1]);

    f.render_widget(Paragraph::new(vec![
        Line::from(vec![Span::raw(" B:"), Span::styled(format!("{}", d.buys_24h), Style::default().fg(Color::Green)),
                        Span::raw(" S:"), Span::styled(format!("{}", d.sells_24h), Style::default().fg(Color::Red))]),
        Line::from(format!(" Tot {}", d.txns)),
    ]).block(Block::default().borders(Borders::ALL).title(" Trades ").border_style(Style::default().fg(Color::DarkGray))), cols[2]);

    // Candles
    render_candles(f, outer[3], &app.candles, app.timeframe);

    // Gauge
    let total = (d.buys_24h + d.sells_24h) as f64;
    let bp = if total > 0.0 { d.buys_24h as f64 / total } else { 0.5 };
    let gc = if bp > 0.55 {Color::Green} else if bp < 0.45 {Color::Red} else {Color::Yellow};
    f.render_widget(Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Buy Pressure ").border_style(Style::default().fg(Color::DarkGray)))
        .gauge_style(Style::default().fg(gc).bg(Color::DarkGray))
        .ratio(bp).label(format!("{}% buy", (bp*100.0) as u32)), outer[4]);

    // Footer
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)), Span::raw(":quit "),
        Span::styled("t", Style::default().fg(Color::Yellow)), Span::raw(":timeframe "),
        Span::styled("r", Style::default().fg(Color::Yellow)), Span::raw(":refresh "),
        Span::raw(format!(" │ [{}] │ tick {} │ GeckoTerminal OHLCV", app.timeframe.label(), app.tick)),
    ])).style(Style::default().fg(Color::DarkGray)), outer[5]);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let app = Arc::new(Mutex::new(App::new()));

    // Initial candle fetch
    let ac = app.clone();
    tokio::spawn(async move {
        // Load candles immediately
        let tf = ac.lock().await.timeframe;
        match fetch_candles(tf).await {
            Ok(c) => { let mut a = ac.lock().await; a.status = format!("{} candles loaded", c.len()); a.candles = c; }
            Err(e) => { ac.lock().await.status = format!("err: {}", e); }
        }
        // Then poll price + candles
        loop {
            if let Ok(data) = fetch_price().await { ac.lock().await.data = data; }
            let tf = ac.lock().await.timeframe;
            if let Ok(c) = fetch_candles(tf).await {
                let mut a = ac.lock().await;
                a.candles = c;
                a.tick += 1;
                a.status = format!("{} candles │ live", a.candles.len());
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // Ticker scroll
    let ac2 = app.clone();
    tokio::spawn(async move {
        loop { ac2.lock().await.ticker_offset += 1; tokio::time::sleep(Duration::from_millis(150)).await; }
    });

    loop {
        { let a = app.lock().await; term.draw(|f| draw(f, &a))?; if a.quit { break; } }
        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.lock().await.quit = true,
                        KeyCode::Char('t') => {
                            let mut a = app.lock().await;
                            a.timeframe = a.timeframe.next();
                            a.candles.clear();
                            a.status = format!("Switching to {}...", a.timeframe.label());
                        }
                        KeyCode::Char('r') => {
                            let mut a = app.lock().await;
                            a.status = "Refreshing...".into();
                            a.candles.clear();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    Ok(())
}
