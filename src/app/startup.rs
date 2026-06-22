// TUI を起動する前に走る前処理をまとめる。
//   - `--list-city`: 地点検索結果を stdout に出して終了
//   - `--city` / `--lat` / `--lon`: Config を上書き
//   - `--save`: 上書き後の Config を保存
//   - `--dump`: TUI を立ち上げずに取得結果を stdout に出して終了
//
// 戻り値が `Ok(None)` の場合は「ここで処理完了、TUI 起動は不要」を意味する。
// `Ok(Some(config))` ならその Config で TUI を起動する。

use std::sync::Arc;

use anyhow::Result;

use crate::api::WeatherProvider;
use crate::cli::Args;
use crate::config::Config;

/// TUI 起動前の CLI 前処理。
///
/// 早期終了系のサブコマンド (`--list-city`, `--dump`) を捌いたら `None` を返す。
/// そうでなければ、CLI で上書き済みの Config を返す。
pub async fn prepare(args: &Args) -> Result<Option<Config>> {
    // 1) 設定読込（無ければデフォルト）
    let mut config = Config::load_or_default()?;

    // 1.5) --lang は他の処理に先んじて反映（geocoding の language にも使うため）
    if let Some(lang) = args.lang {
        config.ui.language = lang;
    }

    // --list-city: 候補を表示して終了
    if let Some(query) = args.list_city.as_ref() {
        list_city(query, config.ui.language).await?;
        return Ok(None);
    }

    // 2) CLI 引数で上書き
    if let Some(city) = &args.city {
        apply_city_override(&mut config, city).await;
    }
    if let (Some(lat), Some(lon)) = (args.lat, args.lon) {
        apply_latlon_override(&mut config, lat, lon, args.city.is_some());
    }

    // 2.5) --save: ここまでで決まった設定を ~/.config/termrain/config.toml に保存
    //      （CLI 引数で指定した内容を次回以降のデフォルトにする）
    if args.save {
        if let Err(e) = config.save() {
            eprintln!("設定の保存に失敗: {e:#}");
        } else if let Some(p) = Config::path() {
            eprintln!("設定を保存しました: {}", p.display());
        }
    }

    Ok(Some(config))
}

/// `--dump`: TUI を立ち上げずに取得結果を stdout に出して終了
pub async fn run_dump(provider: &Arc<dyn WeatherProvider>, config: &Config) -> Result<()> {
    let lat = config.location.latitude;
    let lon = config.location.longitude;
    let cur = provider.current(lat, lon).await?;
    println!("{:#?}", cur);
    let h = provider.hourly(lat, lon).await?;
    println!("hourly: {} points", h.len());
    let d = provider.daily(lat, lon).await?;
    println!("daily: {} days", d.len());
    Ok(())
}

async fn list_city(query: &str, language: crate::i18n::Language) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("termrain/0.1")
        .build()?;
    let hits = crate::api::geocoding::search_many(&client, query, language, 10).await?;
    if hits.is_empty() {
        eprintln!("No matches for: {query}");
        return Ok(());
    }
    println!("Candidates for \"{}\":\n", query);
    for (i, h) in hits.iter().enumerate() {
        let place = match (h.admin1.as_deref(), h.country_name.as_deref()) {
            (Some(a), Some(c)) => format!("{}, {}", a, c),
            (Some(a), None) => a.to_string(),
            (None, Some(c)) => c.to_string(),
            _ => String::new(),
        };
        println!(
            "  {:>2}. {:<24} {:<32}  lat={:>8.4}  lon={:>9.4}",
            i + 1,
            h.name,
            place,
            h.latitude,
            h.longitude,
        );
    }
    println!("\nUse one of:");
    if let Some(top) = hits.first() {
        println!(
            "  termrain --lat {:.4} --lon {:.4}",
            top.latitude, top.longitude
        );
    }
    Ok(())
}

async fn apply_city_override(config: &mut Config, city: &str) {
    let client = match reqwest::Client::builder()
        .user_agent("termrain/0.1")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("地点検索クライアントの初期化に失敗: {e:#}");
            return;
        }
    };
    match crate::api::geocoding::search_many(&client, city, config.ui.language, 5).await {
        Ok(hits) if !hits.is_empty() => {
            let hit = &hits[0];
            config.location.name = hit.name.clone();
            config.location.latitude = hit.latitude;
            config.location.longitude = hit.longitude;
            config.location.country = hit.country.clone();
            // 同名候補が複数あれば「他にもこれだけある」と案内
            if hits.len() > 1 {
                let chosen_loc = match (&hit.admin1, &hit.country_name) {
                    (Some(a), Some(c)) => format!("{}, {}", a, c),
                    (Some(a), None) => a.clone(),
                    (None, Some(c)) => c.clone(),
                    _ => String::new(),
                };
                eprintln!(
                    "\"{}\" → {} ({}) を採用。他の候補は `termrain --list-city {}` で確認可。",
                    city, hit.name, chosen_loc, city
                );
            }
        }
        Ok(_) => {
            eprintln!("該当する地点が見つかりません: {city}");
        }
        Err(e) => {
            eprintln!("地点検索に失敗: {e:#}");
        }
    }
}

fn apply_latlon_override(config: &mut Config, lat: f64, lon: f64, city_was_given: bool) {
    config.location.latitude = lat;
    config.location.longitude = lon;
    // 都市名が未指定なら緯度経度ベースの判定で国も切り替え。
    // 名前は「Custom」程度にしておき、座標はヘッダー側で別途表示する。
    if !city_was_given {
        config.location.name = "Custom".into();
        if lat > 24.0 && lat < 46.0 && lon > 122.0 && lon < 146.0 {
            config.location.country = "JP".into();
        } else {
            config.location.country = "".into();
        }
    }
}
