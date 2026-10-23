//! Named positions (live assembly/105–107, 150–161, dom/panel-named-
//! positions-*.txt, dom/assembly-named-position-editing.txt): the 712 px
//! right pane with a table (frozen Name column, one value column per
//! driving mate degree of freedom under its mate's name), an italic "Name"
//! add row, the blue info box while nothing drives the positions, and the
//! footer's "Add named position". Editing a position shows the yellow
//! "Editing <name> [Done]" banner and leaves only Undo / Redo in the
//! toolbar; "Edit named position driver mate…" (mate menu) opens the
//! "Driving mate selector" dialog.

use super::*;
use gpui_kit::component::input::InputEvent;

/// "Driving mate selector": mates listed, their degrees of freedom checked.
pub(crate) struct NpDriver {
    pub mates: Vec<MateId>,
    pub checked: Vec<(MateId, u8)>,
    pub expanded: HashSet<MateId>,
}

/// A table cell being typed in: (row, column) — column 0 is the name.
pub(crate) struct NpCell {
    pub row: usize,
    pub col: usize,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

/// The value indices a mate type can drive (0 = Rz, 1..3 = Tx / Ty / Tz).
fn driven_dofs(t: MateType) -> &'static [u8] {
    match t {
        MateType::Revolute => &[0],
        MateType::Slider => &[3],
        MateType::Cylindrical => &[3, 0],
        MateType::PinSlot => &[1, 0],
        MateType::Planar => &[1, 2, 0],
        MateType::Parallel => &[1, 2, 3, 0],
        _ => &[],
    }
}

/// Column / checkbox label of a degree of freedom: "⟳ - z", "↔ - x".
fn dof_label(ix: u8) -> &'static str {
    match ix {
        0 => "⟳ - z",
        1 => "↔ - x",
        2 => "↔ - y",
        _ => "↔ - z",
    }
}

impl AssemblyView {
    fn units_now(&self, cx: &App) -> noshape_units::UnitPrefs {
        self.doc.read(cx).session.doc.settings.units
    }

    /// A stored value as the table shows it ("0 deg", "12.5 mm").
    fn np_value_text(&self, v: f64, ix: u8, cx: &App) -> String {
        let units = self.units_now(cx);
        if ix == 0 {
            // Onshape's tables write angles as "90 deg".
            return format!("{} deg", noshape_units::format_number(v.to_degrees(), units.angle_precision));
        }
        noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units)
    }

    /// Footer "Add named position": a new row capturing the current state,
    /// then editing mode for it (no dialog).
    pub(super) fn add_named_position_row(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let n = asm.named_positions.len();
        let name = name.unwrap_or_else(|| {
            let mut k = n + 1;
            while asm.named_positions.iter().any(|p| p.name == format!("Named position {k}")) {
                k += 1;
            }
            format!("Named position {k}")
        });
        let placements = self.result.solved.placements.clone();
        let mate_values = self.result.named_position_values(asm);
        self.edit(&format!("Insert : {name}"), cx, |a| a.named_positions.push(NamedPosition { name, placements, mate_values }));
        self.panel = Some(SidePanel::Positions);
        self.ui.np_editing = Some(n);
        cx.notify();
    }

    /// Done in the editing banner: the position keeps the current state.
    pub(super) fn finish_np_editing(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.ui.np_editing.take() else { return };
        self.ui.np_driver = None;
        self.store_np_state(i, cx);
        cx.notify();
    }

    /// Store the current placements and mate values in position `i`.
    fn store_np_state(&mut self, i: usize, cx: &mut Context<Self>) {
        let placements = self.result.solved.placements.clone();
        let Some(asm) = self.assembly(cx) else { return };
        let values = self.result.named_position_values(asm);
        let name = asm.named_positions.get(i).map(|n| n.name.clone()).unwrap_or_default();
        self.edit(&format!("Update named position : {name}"), cx, |a| {
            if let Some(np) = a.named_positions.get_mut(i) {
                np.placements = placements;
                np.mate_values = values;
            }
        });
    }

    /// Mate menu › Edit named position driver mate…: editing mode (a first
    /// position is created when there is none) and the Driving mate
    /// selector with the mate expanded.
    pub(super) fn open_np_driver(&mut self, mate: MateId, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let mut mates: Vec<MateId> = Vec::new();
        for (m, _) in &asm.position_drivers {
            if !mates.contains(m) {
                mates.push(*m);
            }
        }
        if !mates.contains(&mate) {
            mates.push(mate);
        }
        let checked = asm.position_drivers.clone();
        if asm.named_positions.is_empty() {
            self.add_named_position_row(None, cx);
        } else if self.ui.np_editing.is_none() {
            self.ui.np_editing = Some(self.ui.np_applied.unwrap_or(0));
        }
        self.panel = Some(SidePanel::Positions);
        self.ui.np_driver = Some(NpDriver { mates, checked, expanded: [mate].into_iter().collect() });
        cx.notify();
    }

    fn accept_np_driver(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.ui.np_driver.take() else { return };
        // Keep the mates' order, each mate's DOFs in their own order.
        let mut drivers: Vec<(MateId, u8)> = Vec::new();
        for m in &d.mates {
            let t = self.assembly(cx).and_then(|a| a.mate(*m).map(|x| x.mate_type));
            for ix in t.map(driven_dofs).unwrap_or(&[]) {
                if d.checked.contains(&(*m, *ix)) {
                    drivers.push((*m, *ix));
                }
            }
        }
        self.edit("Driving mate selector", cx, |a| a.position_drivers = drivers);
        cx.notify();
    }

    /// Apply a position: its driving values are imposed (the other mates
    /// stay as they are); toast "<name> is applied." and the row turns blue.
    pub(super) fn apply_np(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let Some(np) = asm.named_positions.get(i).cloned() else { return };
        let drivers = asm.position_drivers.clone();
        let hold: Vec<(MateId, usize, f64)> = drivers.iter().filter_map(|(m, ix)| np.mate_values.iter().find(|(x, _)| x == m).map(|(_, v)| (*m, *ix as usize, v[*ix as usize]))).collect();
        let solved = if hold.is_empty() {
            self.result.prepared.solve_named_position(&np)
        } else {
            let mut start: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
            for (id, p) in &np.placements {
                start.insert(*id, *p);
            }
            self.result.prepared.solve(&SolveOptions { start: Some(start), hold_values: hold, ..Default::default() })
        };
        self.apply_solved(solved, cx);
        self.edit(&format!("Apply named position : {}", np.name), cx, |_| {});
        self.ui.np_applied = Some(i);
        self.ui.np_toast = Some(format!("{} is applied.", np.name));
        cx.notify();
    }

    fn duplicate_np(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let Some(mut np) = asm.named_positions.get(i).cloned() else { return };
        let mut k = 1;
        while asm.named_positions.iter().any(|p| p.name == format!("{} ({k})", np.name)) {
            k += 1;
        }
        np.name = format!("{} ({k})", np.name);
        let name = np.name.clone();
        self.edit(&format!("Duplicate named position : {name}"), cx, |a| a.named_positions.insert(i + 1, np));
    }

    fn move_np(&mut self, i: usize, down: bool, cx: &mut Context<Self>) {
        let j = if down { i + 1 } else { i.wrapping_sub(1) };
        self.edit(if down { "Move down" } else { "Move up" }, cx, |a| {
            if j < a.named_positions.len() && i < a.named_positions.len() {
                a.named_positions.swap(i, j);
            }
        });
        for slot in [&mut self.ui.np_applied, &mut self.ui.np_editing] {
            if *slot == Some(i) {
                *slot = Some(j);
            } else if *slot == Some(j) {
                *slot = Some(i);
            }
        }
    }

    fn delete_np(&mut self, i: usize, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.named_positions.get(i).map(|n| n.name.clone())).unwrap_or_default();
        self.edit(&format!("Delete named position : {name}"), cx, |a| {
            if i < a.named_positions.len() {
                a.named_positions.remove(i);
            }
        });
        for slot in [&mut self.ui.np_applied, &mut self.ui.np_editing] {
            *slot = match *slot {
                Some(k) if k == i => None,
                Some(k) if k > i => Some(k - 1),
                s => s,
            };
        }
    }

    /// Start typing in a cell (blue 2 px frame): column 0 renames, the
    /// others set a driving value (row `len` = the "Name" add row).
    fn edit_np_cell(&mut self, row: usize, col: usize, current: String, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(&input, window, move |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                this.commit_np_cell(cx);
                window.focus(&this.focus, cx);
            }
            InputEvent::Blur => this.commit_np_cell(cx),
            _ => {}
        });
        self.ui.np_cell = Some(NpCell { row, col, input, _sub: sub });
        cx.notify();
    }

    fn commit_np_cell(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.ui.np_cell.take() else { return };
        let text = c.input.read(cx).value().trim().to_string();
        let Some(asm) = self.assembly(cx) else { return };
        let n = asm.named_positions.len();
        if c.col == 0 {
            if text.is_empty() {
                cx.notify();
                return;
            }
            if c.row >= n {
                self.add_named_position_row(Some(text), cx);
            } else {
                self.edit(&format!("Rename named position : {text}"), cx, |a| {
                    if let Some(np) = a.named_positions.get_mut(c.row) {
                        np.name = text.clone();
                    }
                });
            }
            cx.notify();
            return;
        }
        let Some((mate, ix)) = asm.position_drivers.get(c.col - 1).copied() else { return };
        let dim = if ix == 0 { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let units = self.units_now(cx);
        let Ok(q) = noshape_units::evaluate_as(&text, dim, &units, &noshape_units::EmptyScope) else {
            cx.notify();
            return;
        };
        let current = self.result.solved.mate_values.get(&mate).copied().unwrap_or_default();
        // Typing in the position being edited moves the assembly there
        // (the edit below stores the solved placements with the value).
        if self.ui.np_editing == Some(c.row) {
            let solved = self.result.prepared.solve(&SolveOptions { hold_values: vec![(mate, ix as usize, q.value)], ..Default::default() });
            self.apply_solved(solved, cx);
        }
        self.edit("Named position value", cx, |a| {
            if let Some(np) = a.named_positions.get_mut(c.row) {
                match np.mate_values.iter_mut().find(|(m, _)| *m == mate) {
                    Some((_, v)) => v[ix as usize] = q.value,
                    None => {
                        let mut v = current;
                        v[ix as usize] = q.value;
                        np.mate_values.push((mate, v));
                    }
                }
            }
        });
        cx.notify();
    }

    fn np_row_menu(&self, i: usize, name: String) -> Vec<MenuEntry> {
        let n = self.ui.np_count;
        let mut m = Vec::new();
        if self.ui.np_applied != Some(i) {
            m.push(item("Apply named position", move |t, _, cx| t.apply_np(i, cx)));
        }
        m.push(item("Edit named position", move |t, _, cx| {
            t.apply_np(i, cx);
            t.ui.np_toast = None;
            t.ui.np_editing = Some(i);
            cx.notify();
        }));
        m.push(item("Duplicate named position", move |t, _, cx| t.duplicate_np(i, cx)));
        m.push(item("Update named position", move |t, _, cx| t.store_np_state(i, cx)));
        if i + 1 < n {
            m.push(item("Move down", move |t, _, cx| t.move_np(i, true, cx)));
        }
        if i > 0 {
            m.push(item("Move up", move |t, _, cx| t.move_np(i, false, cx)));
        }
        m.push(item(format!("Delete {name}"), move |t, _, cx| t.delete_np(i, cx)));
        m
    }

    // ------------------------------------------------------------ rendering

    /// The 712 px Named positions pane.
    pub(super) fn render_np_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(asm) = self.assembly(cx).cloned() else { return div().into_any_element() };
        self.ui.np_count = asm.named_positions.len();
        let border = rgb(0xaaaaaa);
        let head_bg = rgb(0xeaeaea);
        let cell = |w: f32| div().w(px(w)).h(px(34.)).flex_none().flex().items_center().px(px(6.)).border_r_1().border_b_1().border_color(border).text_size(px(14.));
        let drivers = asm.position_drivers.clone();
        let two_level = !drivers.is_empty();
        // Column widths: live's grips (`ui::col_grip`, 140 px until dragged).
        const T: &str = "named-positions";
        let name_w = crate::ui::col_w(T, "name", 140.);
        let dof_key = |m: &MateId, ix: &u8| format!("{m:?}-{ix}");
        let dof_w: Vec<f32> = drivers.iter().map(|(m, ix)| crate::ui::col_w(T, &dof_key(m, ix), 140.)).collect();
        // Header: Name (spanning both levels) + mate name over DOF label.
        let mut head = h_flex().items_start().border_l_1().border_color(border).bg(head_bg).font_weight(FontWeight::BOLD);
        // Live 155 (qa5 NP2): with drivers, "Name" sits in the lower row of a
        // #eaeaea cell; the mate and degree-of-freedom cells are #d7d7d7.
        let driver_bg = rgb(0xd7d7d7);
        head = head.child(
            cell(name_w)
                .relative()
                .h(px(if two_level { 68. } else { 34. }))
                .justify_center()
                .when(two_level, |d| d.items_end().pb(px(9.)))
                .bg(head_bg)
                .child("Name")
                .child(crate::ui::col_grip(T, "name", 140.)),
        );
        let mut i = 0;
        while i < drivers.len() {
            let m = drivers[i].0;
            let span = drivers[i..].iter().take_while(|(x, _)| *x == m).count();
            let mname = asm.mate(m).map(|x| x.name.clone()).unwrap_or_default();
            let mut sub = h_flex();
            for (k, (m, ix)) in drivers[i..i + span].iter().enumerate() {
                sub = sub.child(cell(dof_w[i + k]).relative().justify_center().bg(driver_bg).child(dof_label(*ix)).child(crate::ui::col_grip(T, &dof_key(m, ix), 140.)));
            }
            head = head.child(v_flex().child(cell(dof_w[i..i + span].iter().sum()).justify_center().bg(driver_bg).child(mname)).child(sub));
            i += span;
        }
        let editing_cell = |row: usize, col: usize| self.ui.np_cell.as_ref().filter(|c| c.row == row && c.col == col).map(|c| c.input.clone());
        let mut rows = v_flex().border_l_1().border_color(border);
        for (r, np) in asm.named_positions.iter().enumerate() {
            let applied = self.ui.np_applied == Some(r);
            let view = cx.entity();
            let name = np.name.clone();
            let name_cell = match editing_cell(r, 0) {
                Some(st) => gpui_kit::base::input::input_frame(cell(name_w).border_2().border_color(rgb(0x1651b0)).px(px(1.)).child(Input::new(&st).appearance(false).text_size(px(13.33)).pl(px(3.))).into_any_element()).into_any_element(),
                None => {
                    let n2 = name.clone();
                    cell(name_w)
                        .relative()
                        .when(applied, |d| d.text_color(rgb(0x1651b0)).child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(rgb(0x1651b0))))
                        .child(div().truncate().child(name.clone()))
                        .id(SharedString::from(format!("np-name-{r}")))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_np_cell(r, 0, n2.clone(), window, cx)))
                        .into_any_element()
                }
            };
            let mut row = h_flex().child(name_cell);
            for (c, (m, ix)) in drivers.iter().enumerate() {
                let v = np.mate_values.iter().find(|(x, _)| x == m).map(|(_, v)| v[*ix as usize]).unwrap_or(0.0);
                let text = self.np_value_text(v, *ix, cx);
                row = row.child(match editing_cell(r, c + 1) {
                    Some(st) => gpui_kit::base::input::input_frame(cell(dof_w[c]).border_2().border_color(rgb(0x1651b0)).px(px(1.)).child(Input::new(&st).appearance(false).text_size(px(13.33)).pl(px(3.))).into_any_element()).into_any_element(),
                    None => cell(dof_w[c])
                        .id(SharedString::from(format!("np-v-{r}-{c}")))
                        .child(text.clone())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_np_cell(r, c + 1, text.clone(), window, cx)))
                        .into_any_element(),
                });
            }
            let entries = self.np_row_menu(r, name);
            rows = rows.child(div().id(SharedString::from(format!("np-row-{r}"))).child(row).os_context_menu(move |menu, window, cx| fill_menu(menu, entries.clone(), view.clone(), window, cx)));
        }
        // The italic "Name" add row.
        let n = asm.named_positions.len();
        rows = rows.child(match editing_cell(n, 0) {
            Some(st) => gpui_kit::base::input::input_frame(cell(name_w).border_2().border_color(rgb(0x1651b0)).px(px(1.)).child(Input::new(&st).appearance(false).text_size(px(13.33)).pl(px(3.))).into_any_element()).into_any_element(),
            None => cell(name_w)
                .id("np-add-row")
                .italic()
                .text_color(rgb(0x999999))
                .child("Name")
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_np_cell(n, 0, String::new(), window, cx)))
                .into_any_element(),
        });
        let table = v_flex().items_start().child(head).child(rows);
        crate::ui::right_resizable(v_flex().id("np-pane").relative(), 0.)
            .w(px(crate::ui::right_panel_w()))
            .h_full()
            .occlude()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x333333))
            .child(
                div().pt(px(4.)).child(
                    div()
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_b_1()
                        .border_color(rgb(0x1651b0))
                        .text_size(px(13.33))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0x1651b0))
                        .child("Named positions"),
                ),
            )
            .child(
                v_flex()
                    .id("np-tables")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt(px(2.))
                    .child(table)
                    .when(drivers.is_empty(), |d| {
                        d.child(
                            // Live 150: flush with the table's left edge, 6 px from the right.
                            v_flex()
                                .mr(px(6.))
                                .mt(px(6.))
                                .px(px(14.))
                                .py(px(13.))
                                .rounded(px(3.))
                                .bg(pal.info_fill)
                                .border_1()
                                .border_color(pal.info_outline)
                                .text_size(px(14.))
                                .text_color(pal.info_text)
                                .child(div().font_weight(FontWeight::BOLD).child("Capture variants of the assembly as named positions"))
                                .child("Position instances in the graphics area and define driving mates."),
                        )
                    }),
            )
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .bg(rgb(0xeeeeee))
                    .border_t_1()
                    .border_l_1()
                    .border_color(rgb(0xcccccc))
                    .child(
                        h_flex().flex_1().justify_center().child(
                            div()
                                .id("np-add")
                                .h(px(27.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .rounded(px(5.25))
                                .border_1()
                                .border_color(rgb(0xaaaaaa))
                                .bg(rgb(0xffffff))
                                .text_size(px(14.))
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip("Add a new named position for this assembly"))
                                .child("Add named position")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_named_position_row(None, cx))),
                        ),
                    )
                    .child(div().pr(px(10.)).child(div().size(px(14.)).rounded_full().bg(rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("?"))),
            )
            .into_any_element()
    }

    /// The yellow `#fab601` "Editing <name> [Done]" banner.
    pub(super) fn render_np_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let i = self.ui.np_editing?;
        let name = self.assembly(cx).and_then(|a| a.named_positions.get(i).map(|n| n.name.clone())).unwrap_or_default();
        Some(yellow_banner(format!("Editing {name}"), cx.listener(|this, _: &ClickEvent, _, cx| this.finish_np_editing(cx))))
    }

    /// The green "✓ <name> is applied. ×" toast.
    pub(super) fn render_np_toast(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = self.ui.np_toast.clone()?;
        let view = cx.entity();
        Some(
            div()
                .absolute()
                .top(px(9.))
                .left_0()
                .right_0()
                // Centered on the window (live), not the graphics area.
                .pr(px(crate::ui::RAIL_W + if self.ui.list_collapsed { 0. } else { crate::ui::left_panel_w() }))
                .flex()
                .justify_center()
                .child(crate::os_ui::message_bubble("np-toast", crate::os_ui::BubbleKind::Success, text, move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.ui.np_toast = None;
                        cx.notify();
                    })
                }, cx))
                .into_any_element(),
        )
    }

    /// The 220 px "Driving mate selector" dialog.
    pub(super) fn render_np_driver(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.np_driver.as_ref()?;
        let pal = p();
        let asm = self.assembly(cx)?.clone();
        let mut list = v_flex().pb(px(2.));
        for m in d.mates.clone() {
            let Some(mate) = asm.mate(m) else { continue };
            let open = d.expanded.contains(&m);
            let name = mate.name.clone();
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("npd-m-{}", m.0)))
                    .h(px(24.))
                    .pl(px(6.))
                    .pr(px(5.))
                    .text_size(px(13.33))
                    .text_color(rgb(0x555555))
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip(format!("{name} (click to {})", if open { "collapse" } else { "expand" })))
                    .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 10., rgb(0x666666)))
                    .child(div().pl(px(4.)).flex_1().truncate().child(name.clone()))
                    .child(
                        div()
                            .id(SharedString::from(format!("npd-x-{}", m.0)))
                            .px(px(5.))
                            .text_size(px(16.67))
                            .text_color(rgb(0x666666))
                            .tooltip(crate::ui::tooltip(format!("Delete {name}")))
                            .child("×")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if let Some(d) = &mut this.ui.np_driver {
                                    d.mates.retain(|x| *x != m);
                                    d.checked.retain(|(x, _)| *x != m);
                                }
                                cx.notify();
                            })),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.ui.np_driver
                            && !d.expanded.remove(&m)
                        {
                            d.expanded.insert(m);
                        }
                        cx.notify();
                    })),
            );
            if open {
                for &ix in driven_dofs(mate.mate_type) {
                    let on = d.checked.contains(&(m, ix));
                    list = list.child(
                        h_flex()
                            .id(SharedString::from(format!("npd-dof-{}-{ix}", m.0)))
                            .h(px(27.))
                            .pl(px(20.))
                            .gap(px(4.))
                            .cursor_pointer()
                            .child(
                                div()
                                    .size(px(12.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_color(if on { rgb(0x1651b0) } else { rgb(0x888888) })
                                    .bg(if on { rgb(0x1651b0) } else { rgb(0xffffff) })
                                    .when(on, |d| d.child(Icon::new(IconName::Check).size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)))),
                            )
                            .child(div().text_size(px(13.33)).text_color(rgb(0x666666)).child(dof_label(ix)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.ui.np_driver {
                                    match d.checked.iter().position(|x| *x == (m, ix)) {
                                        Some(k) => {
                                            d.checked.remove(k);
                                        }
                                        None => d.checked.push((m, ix)),
                                    }
                                }
                                cx.notify();
                            })),
                    );
                }
            }
        }
        Some(
            v_flex()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child("Driving mate selector"))
                        .child(
                            div()
                                .id("npd-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_np_driver(cx))),
                        )
                        .child(
                            div()
                                .id("npd-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.np_driver = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div().p(px(3.)).child(
                        v_flex()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(rgb(0x1651b0))
                            .bg(rgb(0xdef1fb))
                            .child(div().px(px(6.)).pt(px(6.)).pb(px(2.)).text_size(px(12.)).text_color(rgb(0x555555)).child("Mates to drive positions"))
                            .child(list),
                    ),
                )
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(div().size(px(14.)).rounded_full().bg(rgb(0x888888)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("?")))
                .into_any_element(),
        )
    }
}

/// Onshape's yellow editing banner (`#fab601`, 13 px, 28 px tall) with a
/// white "Done" button; shown at the top centre over the toolbar row.
pub(crate) fn yellow_banner(text: String, done: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> AnyElement {
    h_flex()
        .id("yellow-banner")
        .occlude()
        .h(px(28.))
        .pl(px(10.))
        .pr(px(3.))
        .pt(px(3.))
        .items_start()
        .bg(rgb(0xfab601))
        .text_size(px(13.))
        .text_color(rgb(0x333333))
        .child(div().pt(px(1.)).child(text))
        .child(div().w(px(60.)))
        .child(
            div()
                .id("banner-done")
                .h(px(22.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(3.5))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(rgb(0xffffff))
                .cursor_pointer()
                .child("Done")
                .on_click(done),
        )
        .into_any_element()
}
