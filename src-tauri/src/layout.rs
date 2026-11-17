//! 自動整列の配置計算（Win32 に依存しない純粋なロジック）
//!
//! 座標はすべて「見た目の枠」（DWM の不可視リサイズ枠を除いた矩形）で扱う。
//! 実際の `SetWindowPos` 用の矩形への変換は `arrange.rs` が行う。
//!
//! 配置の手順:
//! 1. ワークエリアから固定ウィンドウ（障害物）を除いた空き領域を、重ならない長方形の集まりに分解する
//! 2. 各ウィンドウを、最小サイズが収まり 1 枚あたりの面積が最も広くなる空き領域へ振り分ける
//! 3. 空き領域ごとに、最小サイズを守りつつ見た目が自然になる分割を選んで配置する

use crate::arrange::ArrangeType;
use crate::monitor::MonitorRect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

/// これより小さい空き領域はウィンドウを置かない（物理ピクセル）
const MIN_REGION: Size = Size { w: 240, h: 160 };

/// 「並べて表示」で目標にするセルの縦横比（正方形に近いほど見やすい）
const TARGET_ASPECT: f64 = 1.0;
/// セルの大きさのばらつきに対するペナルティの重み
const UNEVEN_PENALTY: f64 = 0.3;

/// 「重ねて表示」のウィンドウの大きさ（空き領域に対する比率）
const CASCADE_WIDTH_RATIO: f64 = 0.6;
const CASCADE_HEIGHT_RATIO: f64 = 0.7;

fn width(r: &MonitorRect) -> i32 {
    r.right - r.left
}

fn height(r: &MonitorRect) -> i32 {
    r.bottom - r.top
}

fn area(r: &MonitorRect) -> i64 {
    width(r).max(0) as i64 * height(r).max(0) as i64
}

fn rect(left: i32, top: i32, w: i32, h: i32) -> MonitorRect {
    MonitorRect { left, top, right: left + w, bottom: top + h }
}

/// 整列後の各ウィンドウの見た目の矩形を計算する。
///
/// - `area`: 対象モニターのワークエリア
/// - `obstacles`: 動かさないウィンドウ（ピン留め・除外・サイズ変更不可）の矩形
/// - `mins`: 移動対象ウィンドウの最小サイズ。**Z オーダーの前面から順**に並べる
/// - `cascade_offset`: 「重ねて表示」でずらす量（タイトルバーの高さ程度）
///
/// 戻り値は `mins` と同じ順番。
pub fn compute_layout(
    area: &MonitorRect,
    obstacles: &[MonitorRect],
    mins: &[Size],
    arrange_type: ArrangeType,
    cascade_offset: i32,
) -> Vec<MonitorRect> {
    if mins.is_empty() {
        return Vec::new();
    }

    let mut regions = free_regions(area, obstacles, MIN_REGION);
    if regions.is_empty() {
        // 固定ウィンドウが画面をほぼ覆っている場合は、障害物を無視してワークエリア全体を使う
        regions.push(area.clone());
    }

    match arrange_type {
        // 重ねて表示は 1 か所にまとめる方が自然なため、最も広い空き領域だけを使う
        ArrangeType::Cascade => return layout_cascade(&regions[0], mins, cascade_offset),
        // 上下・左右に並べる場合も、離れた場所に 1 枚だけ置かれると並びが崩れて見えるため 1 か所にまとめる
        ArrangeType::Vertical | ArrangeType::Horizontal => regions.truncate(1),
        ArrangeType::Grid => {}
    }

    let mut result = vec![MonitorRect { left: 0, top: 0, right: 0, bottom: 0 }; mins.len()];
    for (region, members) in regions.iter().zip(distribute(&regions, mins)) {
        if members.is_empty() {
            continue;
        }
        let member_mins: Vec<Size> = members.iter().map(|&i| mins[i]).collect();
        let rects = layout_tiles(region, &member_mins, arrange_type);
        for (&i, r) in members.iter().zip(rects) {
            result[i] = r;
        }
    }
    result
}

// ------------------------------------
// 1. 空き領域の分解
// ------------------------------------

/// ワークエリアから障害物を除いた空き領域を、重ならない長方形に分解して面積の大きい順に返す。
///
/// 障害物の辺で区切った格子の上で「空いている最大の長方形」を繰り返し取り出す。
/// `min` より小さい長方形しか残らなくなった時点で打ち切る。
pub fn free_regions(area: &MonitorRect, obstacles: &[MonitorRect], min: Size) -> Vec<MonitorRect> {
    // ワークエリア内に切り詰めた障害物
    let clipped: Vec<MonitorRect> = obstacles
        .iter()
        .map(|o| MonitorRect {
            left: o.left.clamp(area.left, area.right),
            top: o.top.clamp(area.top, area.bottom),
            right: o.right.clamp(area.left, area.right),
            bottom: o.bottom.clamp(area.top, area.bottom),
        })
        .filter(|o| width(o) > 0 && height(o) > 0)
        .collect();

    let mut xs = vec![area.left, area.right];
    let mut ys = vec![area.top, area.bottom];
    for o in &clipped {
        xs.extend([o.left, o.right]);
        ys.extend([o.top, o.bottom]);
    }
    xs.sort_unstable();
    xs.dedup();
    ys.sort_unstable();
    ys.dedup();
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);

    // blocked[y][x]: 格子セルが障害物、または取り出し済みの領域に含まれるか
    let mut blocked = vec![vec![false; nx]; ny];
    for (cy, row) in blocked.iter_mut().enumerate() {
        for (cx, cell) in row.iter_mut().enumerate() {
            let c = MonitorRect { left: xs[cx], top: ys[cy], right: xs[cx + 1], bottom: ys[cy + 1] };
            *cell = clipped.iter().any(|o| intersects(o, &c));
        }
    }

    let mut regions = Vec::new();
    loop {
        // 2 次元累積和で「範囲内に塞がったセルがあるか」を O(1) で判定する
        let mut sum = vec![vec![0u32; nx + 1]; ny + 1];
        for y in 0..ny {
            for x in 0..nx {
                sum[y + 1][x + 1] = sum[y][x + 1] + sum[y + 1][x] - sum[y][x] + blocked[y][x] as u32;
            }
        }
        let is_free = |x0: usize, y0: usize, x1: usize, y1: usize| {
            sum[y1][x1] + sum[y0][x0] == sum[y0][x1] + sum[y1][x0]
        };

        let mut best: Option<(i64, usize, usize, usize, usize)> = None;
        for x0 in 0..nx {
            for x1 in (x0 + 1)..=nx {
                if xs[x1] - xs[x0] < min.w {
                    continue;
                }
                for y0 in 0..ny {
                    for y1 in (y0 + 1)..=ny {
                        if ys[y1] - ys[y0] < min.h {
                            continue;
                        }
                        if !is_free(x0, y0, x1, y1) {
                            break; // 下へ伸ばしても塞がったまま
                        }
                        let a = (xs[x1] - xs[x0]) as i64 * (ys[y1] - ys[y0]) as i64;
                        if best.is_none_or(|(b, ..)| a > b) {
                            best = Some((a, x0, y0, x1, y1));
                        }
                    }
                }
            }
        }

        let Some((_, x0, y0, x1, y1)) = best else { break };
        regions.push(MonitorRect { left: xs[x0], top: ys[y0], right: xs[x1], bottom: ys[y1] });
        for row in blocked.iter_mut().take(y1).skip(y0) {
            for cell in row.iter_mut().take(x1).skip(x0) {
                *cell = true;
            }
        }
    }
    regions
}

fn intersects(a: &MonitorRect, b: &MonitorRect) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

// ------------------------------------
// 2. 空き領域への振り分け
// ------------------------------------

/// 空き領域を必ず使う目安。1 枚あたりの平均面積に対してこの比率以上の広さがある領域は空けておかない
const FILL_REGION_RATIO: f64 = 0.5;

/// 前面のウィンドウから順に領域へ振り分ける。戻り値は領域ごとのウィンドウ番号（前面から順）。
///
/// 1. 十分な広さがある領域には、まず 1 枚ずつ置く（デスクトップに不自然な空きを残さない）
/// 2. 残りは、最小サイズが収まり「1 枚あたりの面積」が最も広くなる領域へ置く
fn distribute(regions: &[MonitorRect], mins: &[Size]) -> Vec<Vec<usize>> {
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); regions.len()];
    let total_area: i64 = regions.iter().map(area).sum();
    let average_share = total_area as f64 / mins.len().max(1) as f64;

    let mut rest = 0..mins.len();
    for (k, region) in regions.iter().enumerate() {
        if area(region) as f64 >= average_share * FILL_REGION_RATIO {
            let Some(i) = rest.next() else { break };
            if width(region) >= mins[i].w && height(region) >= mins[i].h {
                members[k].push(i);
            } else {
                // 最小サイズが収まらないウィンドウは、下の通常の振り分けに回す
                rest = i..mins.len();
                break;
            }
        }
    }

    for i in rest {
        let min = &mins[i];
        let fits = |r: &MonitorRect| width(r) >= min.w && height(r) >= min.h;
        let per_window = |k: usize| area(&regions[k]) as f64 / (members[k].len() + 1) as f64;
        let chosen = (0..regions.len())
            .filter(|&k| fits(&regions[k]))
            .max_by(|&a, &b| per_window(a).total_cmp(&per_window(b)).then(b.cmp(&a)))
            // どこにも収まらなければ最も広い領域へ（重なりは許容する）
            .unwrap_or(0);
        members[chosen].push(i);
    }
    members
}

// ------------------------------------
// 3. 領域内の配置
// ------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    /// 縦の列を左から並べ、列の中を上下に分割する
    Columns,
    /// 横の行を上から並べ、行の中を左右に分割する
    Rows,
}

/// 1 つの空き領域の中にウィンドウをタイル状に並べる。`mins` の順に読み順（左上から）の枠を割り当てる。
fn layout_tiles(region: &MonitorRect, mins: &[Size], arrange_type: ArrangeType) -> Vec<MonitorRect> {
    let n = mins.len();
    let candidates: Vec<(Orientation, usize)> = match arrange_type {
        // 上下に並べる: 1 列が基本。最小サイズが収まらなければ列を増やす
        ArrangeType::Vertical => (1..=n).map(|lines| (Orientation::Columns, lines)).collect(),
        // 左右に並べる: 1 行が基本。最小サイズが収まらなければ行を増やす
        ArrangeType::Horizontal => (1..=n).map(|lines| (Orientation::Rows, lines)).collect(),
        // 並べて表示: すべての分割の中から最も自然なものを選ぶ
        _ => [Orientation::Columns, Orientation::Rows]
            .into_iter()
            .flat_map(|o| (1..=n).map(move |lines| (o, lines)))
            .collect(),
    };

    let mut best: Option<(i64, f64, Vec<MonitorRect>)> = None;
    for (orientation, lines) in candidates {
        let counts = balanced_counts(n, lines);
        let (rects, overflow) = layout_lines(region, mins, &counts, orientation);
        let score = match arrange_type {
            // 上下・左右は、収まる限り線の数が少ないもの（＝最初に見つかったもの）を優先する
            ArrangeType::Vertical | ArrangeType::Horizontal => lines as f64,
            _ => grid_penalty(&rects),
        };
        let better = match &best {
            None => true,
            Some((best_overflow, best_score, _)) => {
                overflow < *best_overflow || (overflow == *best_overflow && score < *best_score - 1e-9)
            }
        };
        if better {
            best = Some((overflow, score, rects));
        }
    }
    best.map(|(_, _, rects)| rects).unwrap_or_default()
}

/// n 枚を lines 本の線に、少ない線が先頭に来るように均等に配る（例: 5 枚 3 本 → [1, 2, 2]）。
/// 先頭の線ほど 1 枚あたりの枠が大きくなるため、前面のウィンドウが大きい枠に入る。
fn balanced_counts(n: usize, lines: usize) -> Vec<usize> {
    let base = n / lines;
    let extra = n % lines;
    (0..lines).map(|i| if i >= lines - extra { base + 1 } else { base }).collect()
}

/// 線（列または行）ごとに分割して矩形を作る。戻り値の 2 つ目は最小サイズに足りなかった合計ピクセル数。
fn layout_lines(
    region: &MonitorRect,
    mins: &[Size],
    counts: &[usize],
    orientation: Orientation,
) -> (Vec<MonitorRect>, i64) {
    // 線の並ぶ方向（主軸）と、線の中で分割する方向（副軸）の長さと最小値を取り出す
    let (main_total, cross_total) = match orientation {
        Orientation::Columns => (width(region), height(region)),
        Orientation::Rows => (height(region), width(region)),
    };
    let main_min = |s: &Size| if orientation == Orientation::Columns { s.w } else { s.h };
    let cross_min = |s: &Size| if orientation == Orientation::Columns { s.h } else { s.w };

    let mut groups = Vec::with_capacity(counts.len());
    let mut next = 0;
    for &c in counts {
        groups.push(&mins[next..next + c]);
        next += c;
    }

    let line_mins: Vec<i32> = groups.iter().map(|g| g.iter().map(main_min).max().unwrap_or(0)).collect();
    let (line_sizes, mut overflow) = split_with_minimums(main_total, &line_mins);

    let mut rects = Vec::with_capacity(mins.len());
    let mut main_pos = 0;
    for (group, line_size) in groups.iter().zip(line_sizes) {
        let cell_mins: Vec<i32> = group.iter().map(cross_min).collect();
        let (cell_sizes, cell_overflow) = split_with_minimums(cross_total, &cell_mins);
        overflow += cell_overflow;
        let mut cross_pos = 0;
        for cell_size in cell_sizes {
            rects.push(match orientation {
                Orientation::Columns => rect(region.left + main_pos, region.top + cross_pos, line_size, cell_size),
                Orientation::Rows => rect(region.left + cross_pos, region.top + main_pos, cell_size, line_size),
            });
            cross_pos += cell_size;
        }
        main_pos += line_size;
    }
    (rects, overflow)
}

/// total を均等に分割する。最小値に満たない要素は最小値で固定し、残りを他の要素で均等に分ける。
/// 最小値の合計が total を超える場合は均等に分割し、不足分を 2 つ目の戻り値で返す。
pub fn split_with_minimums(total: i32, mins: &[i32]) -> (Vec<i32>, i64) {
    let n = mins.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    let min_sum: i64 = mins.iter().map(|&m| m.max(0) as i64).sum();
    if min_sum > total as i64 {
        return (equal_split(total, n), min_sum - total as i64);
    }

    let mut fixed = vec![false; n];
    loop {
        let fixed_sum: i32 = (0..n).filter(|&i| fixed[i]).map(|i| mins[i]).sum();
        let free = (0..n).filter(|&i| !fixed[i]).count();
        let share = (total - fixed_sum) / free.max(1) as i32;
        let newly_fixed: Vec<usize> = (0..n).filter(|&i| !fixed[i] && mins[i] > share).collect();
        if newly_fixed.is_empty() || free == newly_fixed.len() {
            let free_sizes = equal_split(total - fixed_sum, free);
            let mut free_iter = free_sizes.into_iter();
            let sizes = (0..n)
                .map(|i| if fixed[i] { mins[i] } else { free_iter.next().unwrap_or(0) })
                .collect();
            return (sizes, 0);
        }
        for i in newly_fixed {
            fixed[i] = true;
        }
    }
}

/// 端数を先頭から 1 ピクセルずつ配って、合計がちょうど total になるように均等分割する
fn equal_split(total: i32, n: usize) -> Vec<i32> {
    if n == 0 {
        return Vec::new();
    }
    let base = total / n as i32;
    let extra = (total % n as i32) as usize;
    (0..n).map(|i| base + (i < extra) as i32).collect()
}

/// 「並べて表示」の分割の不自然さ。セルが正方形から離れるほど、大きさがばらつくほど大きくなる。
fn grid_penalty(rects: &[MonitorRect]) -> f64 {
    if rects.is_empty() {
        return 0.0;
    }
    let aspect: f64 = rects
        .iter()
        .map(|r| ((width(r).max(1) as f64 / height(r).max(1) as f64).ln() - TARGET_ASPECT.ln()).abs())
        .sum::<f64>()
        / rects.len() as f64;
    let areas = rects.iter().map(|r| area(r).max(1) as f64);
    let max = areas.clone().fold(f64::MIN, f64::max);
    let min = areas.fold(f64::MAX, f64::min);
    aspect + UNEVEN_PENALTY * (max / min - 1.0)
}

/// 重ねて表示。仕様 (SPEC 2-5) どおり、Z オーダーの背面のウィンドウほど左上（奥）に置く。
fn layout_cascade(region: &MonitorRect, mins: &[Size], offset: i32) -> Vec<MonitorRect> {
    let n = mins.len();
    let offset = offset.max(1);
    let base_w = (width(region) as f64 * CASCADE_WIDTH_RATIO).round() as i32;
    let base_h = (height(region) as f64 * CASCADE_HEIGHT_RATIO).round() as i32;
    let max_min_w = mins.iter().map(|s| s.w).max().unwrap_or(0);
    let max_min_h = mins.iter().map(|s| s.h).max().unwrap_or(0);
    // 全ウィンドウの最小サイズを満たし、かつ領域からはみ出さない大きさ
    let w = base_w.max(max_min_w).min(width(region));
    let h = base_h.max(max_min_h).min(height(region));

    // 右下へずらせる段数。超えたら左上に戻り、少し右へずらして次の段を重ねる
    let steps = (((width(region) - w).min(height(region) - h) / offset) as usize + 1).clamp(1, n);

    (0..n)
        .map(|i| {
            let k = n - 1 - i; // 背面から数えた順番
            let (lap, step) = (k / steps, k % steps);
            let x = (region.left + step as i32 * offset + lap as i32 * offset * 2).min(region.right - w);
            let y = region.top + step as i32 * offset;
            rect(x, y, w, h)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: MonitorRect = MonitorRect { left: 0, top: 0, right: 1920, bottom: 1080 };
    const NO_MIN: Size = Size { w: 0, h: 0 };

    fn r(left: i32, top: i32, right: i32, bottom: i32) -> MonitorRect {
        MonitorRect { left, top, right, bottom }
    }

    fn grid(n: usize) -> Vec<MonitorRect> {
        compute_layout(&FHD, &[], &vec![NO_MIN; n], ArrangeType::Grid, 30)
    }

    /// どの 2 枚も重ならず、すべて領域内に収まっていること
    fn assert_tiled(rects: &[MonitorRect], area: &MonitorRect) {
        for (i, a) in rects.iter().enumerate() {
            assert!(a.left >= area.left && a.top >= area.top && a.right <= area.right && a.bottom <= area.bottom, "{:?} は領域外", a);
            for b in &rects[i + 1..] {
                assert!(!intersects(a, b), "{:?} と {:?} が重なっている", a, b);
            }
        }
    }

    #[test]
    fn test_split_with_minimums() {
        assert_eq!(split_with_minimums(100, &[0, 0, 0]), (vec![34, 33, 33], 0));
        // 最小値 60 の要素を固定し、残りを均等に分ける
        assert_eq!(split_with_minimums(100, &[60, 0, 0]), (vec![60, 20, 20], 0));
        // 収まらない場合は均等分割し、不足分を返す
        assert_eq!(split_with_minimums(100, &[60, 60]), (vec![50, 50], 20));
    }

    #[test]
    fn test_balanced_counts_puts_fewer_first() {
        assert_eq!(balanced_counts(5, 3), vec![1, 2, 2]);
        assert_eq!(balanced_counts(4, 2), vec![2, 2]);
        assert_eq!(balanced_counts(3, 1), vec![3]);
    }

    #[test]
    fn test_grid_natural_shapes_on_full_hd() {
        // 1 枚: 全面
        assert_eq!(grid(1), vec![FHD]);
        // 2 枚: 左右に半分ずつ
        assert_eq!(grid(2), vec![r(0, 0, 960, 1080), r(960, 0, 1920, 1080)]);
        // 3 枚: 3 列
        assert_eq!(grid(3), vec![r(0, 0, 640, 1080), r(640, 0, 1280, 1080), r(1280, 0, 1920, 1080)]);
        // 4 枚: 2×2
        let four = grid(4);
        assert!(four.iter().all(|c| width(c) == 960 && height(c) == 540), "{:?}", four);
        // 6 枚: 3×2
        let six = grid(6);
        assert!(six.iter().all(|c| width(c) == 640 && height(c) == 540), "{:?}", six);
        for n in 1..=9 {
            assert_tiled(&grid(n), &FHD);
        }
    }

    #[test]
    fn test_grid_leaves_no_empty_cell() {
        // 5 枚でも空きセルを作らず、領域を埋め切る
        let five = grid(5);
        let total: i64 = five.iter().map(area).sum();
        assert_eq!(total, area(&FHD));
        assert_tiled(&five, &FHD);
    }

    #[test]
    fn test_grid_portrait_monitor_stacks_vertically() {
        let portrait = r(0, 0, 1080, 1920);
        let rects = compute_layout(&portrait, &[], &[NO_MIN; 2], ArrangeType::Grid, 30);
        assert_eq!(rects, vec![r(0, 0, 1080, 960), r(0, 960, 1080, 1920)]);
    }

    #[test]
    fn test_free_regions_l_shape() {
        // 右上に固定ウィンドウ → 左の縦長と右下の 2 領域に分かれる
        let obstacle = r(1280, 0, 1920, 540);
        let regions = free_regions(&FHD, &[obstacle], MIN_REGION);
        assert_eq!(regions, vec![r(0, 0, 1280, 1080), r(1280, 540, 1920, 1080)]);
    }

    #[test]
    fn test_free_regions_ignore_slivers_and_clip_outside() {
        // ワークエリア外にはみ出した障害物は切り詰め、細すぎる隙間は使わない
        let obstacle = r(-100, -100, 1800, 1200);
        assert!(free_regions(&FHD, &[obstacle], MIN_REGION).is_empty());
    }

    #[test]
    fn test_layout_avoids_obstacles() {
        let obstacles = [r(1280, 0, 1920, 540), r(0, 900, 300, 1080)];
        for typ in [ArrangeType::Grid, ArrangeType::Vertical, ArrangeType::Horizontal] {
            for n in 1..=6 {
                let rects = compute_layout(&FHD, &obstacles, &vec![NO_MIN; n], typ, 30);
                assert_eq!(rects.len(), n);
                assert_tiled(&rects, &FHD);
                for rc in &rects {
                    for o in &obstacles {
                        assert!(!intersects(rc, o), "{:?} が固定ウィンドウ {:?} に重なっている ({:?}, {}枚)", rc, o, typ, n);
                    }
                }
            }
        }
    }

    #[test]
    fn test_grid_uses_every_large_enough_region() {
        // 右上に固定ウィンドウ。3 枚なら右下の空きにも 1 枚置き、デスクトップに大きな空きを残さない
        let obstacle = r(1300, 0, 1920, 420);
        let bottom_right = r(1300, 420, 1920, 1080);
        let rects = compute_layout(&FHD, &[obstacle], &[NO_MIN; 3], ArrangeType::Grid, 30);
        assert_eq!(rects.iter().filter(|c| **c == bottom_right).count(), 1, "{:?}", rects);
        // 前面のウィンドウは最も広い領域に入る
        assert_eq!(rects[0].left, 0);
    }

    #[test]
    fn test_grid_skips_small_region_for_few_windows() {
        // 2 枚なら、平均の半分に満たない狭い空きは使わない
        let obstacle = r(1300, 0, 1920, 600);
        let rects = compute_layout(&FHD, &[obstacle], &[NO_MIN; 2], ArrangeType::Grid, 30);
        assert!(rects.iter().all(|c| c.right <= 1300), "{:?}", rects);
    }

    #[test]
    fn test_vertical_keeps_windows_in_one_column() {
        // 左下に固定ウィンドウがあっても、上下に並べる場合は 1 つの領域にまとめる
        let obstacle = r(0, 700, 500, 1080);
        let rects = compute_layout(&FHD, &[obstacle], &[NO_MIN; 3], ArrangeType::Vertical, 30);
        assert!(rects.iter().all(|c| c.left == rects[0].left && c.right == rects[0].right), "{:?}", rects);
        assert_tiled(&rects, &FHD);
    }

    #[test]
    fn test_fully_covered_falls_back_to_whole_area() {
        // 最大化された固定ウィンドウで空きが無い場合は、ワークエリア全体に並べる
        let rects = compute_layout(&FHD, &[FHD], &[NO_MIN; 2], ArrangeType::Grid, 30);
        assert_eq!(rects, vec![r(0, 0, 960, 1080), r(960, 0, 1920, 1080)]);
    }

    #[test]
    fn test_horizontal_wraps_when_min_width_does_not_fit() {
        // 最小幅 700 の 4 枚は 1 行に収まらないので 2 行に折り返す
        let mins = [Size { w: 700, h: 0 }; 4];
        let rects = compute_layout(&FHD, &[], &mins, ArrangeType::Horizontal, 30);
        assert_tiled(&rects, &FHD);
        assert!(rects.iter().all(|c| width(c) >= 700 && height(c) == 540), "{:?}", rects);
    }

    #[test]
    fn test_vertical_is_single_column_when_it_fits() {
        let rects = compute_layout(&FHD, &[], &[NO_MIN; 3], ArrangeType::Vertical, 30);
        assert_eq!(rects, vec![r(0, 0, 1920, 360), r(0, 360, 1920, 720), r(0, 720, 1920, 1080)]);
    }

    #[test]
    fn test_grid_respects_minimum_size() {
        // 前面のウィンドウが最小幅 1200 → その列を 1200 にして残りを他の列で分ける
        let mins = [Size { w: 1200, h: 0 }, NO_MIN];
        let rects = compute_layout(&FHD, &[], &mins, ArrangeType::Grid, 30);
        assert_tiled(&rects, &FHD);
        assert!(width(&rects[0]) >= 1200, "{:?}", rects);
    }

    #[test]
    fn test_frontmost_gets_largest_cell() {
        // 3 枚を 2 列 [1, 2] に分けた場合、前面のウィンドウが大きい 1 枚の列に入る
        let (rects, _) = layout_lines(&FHD, &[NO_MIN; 3], &balanced_counts(3, 2), Orientation::Columns);
        assert_eq!(rects[0], r(0, 0, 960, 1080));
        assert!(area(&rects[0]) > area(&rects[1]));
    }

    #[test]
    fn test_window_too_large_for_any_region_goes_to_largest() {
        let obstacle = r(1280, 0, 1920, 540);
        let regions = free_regions(&FHD, &[obstacle], MIN_REGION);
        let members = distribute(&regions, &[Size { w: 1500, h: 900 }]);
        assert_eq!(members[0], vec![0]);
    }

    #[test]
    fn test_cascade_backmost_is_top_left_and_stays_inside() {
        let rects = compute_layout(&FHD, &[], &[NO_MIN; 3], ArrangeType::Cascade, 30);
        // mins は前面から順なので、最後（最背面）が左上
        assert_eq!((rects[2].left, rects[2].top), (0, 0));
        assert_eq!((rects[1].left, rects[1].top), (30, 30));
        assert_eq!((rects[0].left, rects[0].top), (60, 60));
        // 多数でも領域からはみ出さない
        for rc in compute_layout(&FHD, &[], &[NO_MIN; 30], ArrangeType::Cascade, 30) {
            assert!(rc.right <= FHD.right && rc.bottom <= FHD.bottom, "{:?}", rc);
        }
    }

    #[test]
    fn test_cascade_uses_minimum_size() {
        let rects = compute_layout(&FHD, &[], &[Size { w: 1500, h: 900 }, NO_MIN], ArrangeType::Cascade, 30);
        assert!(rects.iter().all(|c| width(c) >= 1500 && height(c) >= 900));
    }
}
