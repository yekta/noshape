//! Tests of view links between sheets (moved from noshape-model-data: they move views, which needs view geometry).

use crate::drawing::*;
use crate::drawing::{ViewKind, ViewOrientation};

fn base(id: u64) -> DrawingView {
    DrawingView { id: ViewId(id), kind: ViewKind::Base { source: crate::document::TabId(1), config: Default::default(), orientation: ViewOrientation::Front, part: None }, position: [100.0, 100.0], ..Default::default() }
}

#[test]
fn moving_a_parent_leaves_its_children_linked() {
    let mut d = Drawing::default();
    let s2 = d.add_sheet(0);
    d.sheets[0].views.push(base(1));
    d.sheets[0].views.push(DrawingView { id: ViewId(2), kind: ViewKind::Projected { parent: ViewId(1), direction: Some([0.0, 1.0]) }, position: [100.0, 180.0], ..Default::default() });
    d.sync_links();
    // Move only the parent to sheet 2.
    let map = d.move_views_to_sheet_linked(0, &[ViewId(1)], s2).unwrap();
    let moved = map[0].1;
    assert!(d.sheets[0].view(ViewId(2)).is_some(), "the child stays");
    let (uid, pid) = d.sheets[0].linked_parent(ViewId(2)).expect("linked across sheets");
    assert_eq!((d.sheet_by_uid(uid), pid), (Some(s2), moved));
    // A parent mirror on sheet 1, a child mirror on sheet 2.
    assert_eq!(d.sheets[0].views.iter().filter(|v| v.id.is_ghost()).count(), 1);
    assert_eq!(d.sheets[s2].views.iter().filter(|v| v.id.is_child_ghost()).count(), 1);
    // The parent's changes reach the mirror.
    d.sheets[s2].view_mut(moved).unwrap().scale = Some(2.0);
    d.sync_links();
    let g = d.sheets[0].view(d.sheets[0].parent_of(ViewId(2)).unwrap()).unwrap();
    assert_eq!(g.scale, Some(2.0));
    assert_eq!(crate::drawing::views::view_scale(&d.sheets[0], ViewId(2)), 2.0);
    // Moving it back: no links left.
    d.move_views_to_sheet_linked(s2, &[moved], 0).unwrap();
    assert!(d.sheets.iter().all(|s| s.views.iter().all(|v| !v.id.is_ghost())));
    // Deleting a linked view's parent deletes the linked view.
    let s2b = d.add_sheet(0);
    let back = d.sheets[0].real_views().find(|v| matches!(v.kind, ViewKind::Base { .. })).unwrap().id;
    d.move_views_to_sheet_linked(0, &[back], s2b).unwrap();
    let p = d.sheets[s2b].real_views().next().unwrap().id;
    d.sheets[s2b].remove_view(p);
    d.sync_links();
    assert!(d.sheets[0].real_views().next().is_none());
}
