/// What a saved tab remembers about the window it came from.
pub struct TabHint<'a> {
    pub exe_path: &'a str,
    pub title: &'a str,
    pub pid: Option<u32>,
    pub rect: Option<[i32; 4]>,
}

/// A window a saved tab can be given back to.
pub struct Target {
    pub hwnd: isize,
    pub exe_path: String,
    pub title: String,
    pub pid: u32,
    pub rect: Option<[i32; 4]>,
}

const NEAR: i32 = 12;

/// Pairs saved tabs with windows, best scores claiming first, so a title-less tab cannot steal another tab's match.
pub fn match_tabs(tabs: &[TabHint<'_>], targets: &[Target]) -> Vec<(usize, isize)> {
    let mut scored: Vec<(u8, usize, usize)> = Vec::new();
    for (ix, tab) in tabs.iter().enumerate() {
        for (ti, target) in targets.iter().enumerate() {
            if let Some(score) = score(tab, target) {
                scored.push((score, ix, ti));
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let mut taken_tabs = vec![false; tabs.len()];
    let mut taken_targets = vec![false; targets.len()];
    let mut pairs = Vec::with_capacity(tabs.len());
    for (_, ix, ti) in scored {
        if taken_tabs[ix] || taken_targets[ti] {
            continue;
        }
        taken_tabs[ix] = true;
        taken_targets[ti] = true;
        pairs.push((ix, targets[ti].hwnd));
    }
    pairs.sort_by_key(|(ix, _)| *ix);
    pairs
}

/// How well a window fits a saved tab; none when the exe differs.
fn score(tab: &TabHint<'_>, target: &Target) -> Option<u8> {
    if !target.exe_path.eq_ignore_ascii_case(tab.exe_path) {
        return None;
    }
    let mut score = 1;
    if tab.title == target.title {
        score += 4;
    }
    if tab.pid == Some(target.pid) {
        score += 2;
    }
    if near(tab.rect, target.rect) {
        score += 1;
    }
    Some(score)
}

fn near(a: Option<[i32; 4]>, b: Option<[i32; 4]>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    // Widened, so two distant rects cannot overflow the subtraction.
    a.iter().zip(b.iter()).all(|(x, y)| {
        let gap = i64::from(*x) - i64::from(*y);
        gap.abs() <= i64::from(NEAR)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab<'a>(exe: &'a str, title: &'a str) -> TabHint<'a> {
        TabHint {
            exe_path: exe,
            title,
            pid: None,
            rect: None,
        }
    }

    fn target(hwnd: isize, exe: &str, title: &str) -> Target {
        Target {
            hwnd,
            exe_path: exe.to_string(),
            title: title.to_string(),
            pid: 1,
            rect: None,
        }
    }

    #[test]
    fn title_beats_saved_order() {
        let tabs = [
            tab("notepad.exe", "untitled - notepad"),
            tab("notepad.exe", "notes.txt - notepad"),
        ];
        let targets = [
            target(10, "notepad.exe", "notes.txt - notepad"),
            target(20, "notepad.exe", "budget.txt - notepad"),
        ];
        assert_eq!(match_tabs(&tabs, &targets), vec![(0, 20), (1, 10)]);
    }

    #[test]
    fn process_breaks_a_title_tie() {
        let mut tabs = [
            tab("explorer.exe", "downloads"),
            tab("explorer.exe", "downloads"),
        ];
        tabs[0].pid = Some(7);
        tabs[1].pid = Some(9);
        let mut targets = [
            target(1, "explorer.exe", "downloads"),
            target(2, "explorer.exe", "downloads"),
        ];
        targets[0].pid = 9;
        targets[1].pid = 7;
        assert_eq!(match_tabs(&tabs, &targets), vec![(0, 2), (1, 1)]);
    }

    #[test]
    fn place_breaks_a_title_tie() {
        let mut tabs = [
            tab("explorer.exe", "downloads"),
            tab("explorer.exe", "downloads"),
        ];
        tabs[0].rect = Some([0, 0, 800, 600]);
        tabs[1].rect = Some([900, 0, 800, 600]);
        let mut targets = [
            target(1, "explorer.exe", "downloads"),
            target(2, "explorer.exe", "downloads"),
        ];
        targets[0].rect = Some([900, 0, 800, 600]);
        targets[1].rect = Some([0, 0, 800, 600]);
        assert_eq!(match_tabs(&tabs, &targets), vec![(0, 2), (1, 1)]);
    }

    #[test]
    fn one_window_takes_one_tab() {
        let tabs = [tab("code.exe", "a"), tab("code.exe", "b")];
        let targets = [target(1, "code.exe", "a")];
        assert_eq!(match_tabs(&tabs, &targets), vec![(0, 1)]);
    }

    #[test]
    fn other_exes_are_ignored() {
        let tabs = [tab("wt.exe", "powershell")];
        let targets = [target(1, "notepad.exe", "powershell")];
        assert!(match_tabs(&tabs, &targets).is_empty());
    }

    #[test]
    fn exe_match_ignores_case() {
        let tabs = [tab("C:\\Windows\\NOTEPAD.EXE", "x")];
        let targets = [target(7, "c:\\windows\\notepad.exe", "x")];
        assert_eq!(match_tabs(&tabs, &targets), vec![(0, 7)]);
    }
}
