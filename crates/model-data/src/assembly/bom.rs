//! The BOM panel settings stored with an Assembly (the BOM itself is computed in noshape-model).

use super::BomBehavior;
use serde::{Deserialize, Serialize};

/// "BOM type" (native select: Flattened, Structured).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BomType {
    #[default]
    Flattened,
    Structured,
}

impl BomType {
    pub const ALL: [BomType; 2] = [BomType::Flattened, BomType::Structured];
    pub fn label(self) -> &'static str {
        match self {
            BomType::Flattened => "Flattened",
            BomType::Structured => "Structured",
        }
    }
}

/// A BOM table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BomColumn {
    Item,
    Quantity,
    PartNumber,
    Description,
    Appearance,
    Name,
    Revision,
    State,
    Vendor,
    Project,
    ProductLine,
    Material,
    Title1,
    Title2,
    Title3,
    NotRevisionManaged,
    UnitOfMeasure,
    Mass,
    SubassemblyBomBehavior,
    TessellationQuality,
}

/// How a column's cells are edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellEdit {
    /// Computed or locked (Item, Quantity, State, Material, Mass, …).
    ReadOnly,
    Text,
    Check,
    /// A list of choices (`BomColumn::choices`).
    Choice,
}

impl BomColumn {
    /// The default table (live 181).
    pub const DEFAULT: [BomColumn; 4] = [BomColumn::Item, BomColumn::Quantity, BomColumn::PartNumber, BomColumn::Description];
    /// "Add column ▾" list, in Onshape's order (live 183).
    pub const ADDABLE: [BomColumn; 16] = [
        BomColumn::Appearance,
        BomColumn::Name,
        BomColumn::Revision,
        BomColumn::State,
        BomColumn::Vendor,
        BomColumn::Project,
        BomColumn::ProductLine,
        BomColumn::Material,
        BomColumn::Title1,
        BomColumn::Title2,
        BomColumn::Title3,
        BomColumn::NotRevisionManaged,
        BomColumn::UnitOfMeasure,
        BomColumn::Mass,
        BomColumn::SubassemblyBomBehavior,
        BomColumn::TessellationQuality,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BomColumn::Item => "Item",
            BomColumn::Quantity => "Quantity",
            BomColumn::PartNumber => "Part number",
            BomColumn::Description => "Description",
            BomColumn::Appearance => "Appearance",
            BomColumn::Name => "Name",
            BomColumn::Revision => "Revision",
            BomColumn::State => "State",
            BomColumn::Vendor => "Vendor",
            BomColumn::Project => "Project",
            BomColumn::ProductLine => "Product line",
            BomColumn::Material => "Material",
            BomColumn::Title1 => "Title 1",
            BomColumn::Title2 => "Title 2",
            BomColumn::Title3 => "Title 3",
            BomColumn::NotRevisionManaged => "Not revision managed",
            BomColumn::UnitOfMeasure => "Unit of measure",
            BomColumn::Mass => "Mass",
            BomColumn::SubassemblyBomBehavior => "Subassembly BOM behavior",
            BomColumn::TessellationQuality => "Tessellation quality",
        }
    }

    /// Item is the frozen first column (live: `os-colgroup-frozen`).
    pub fn frozen(self) -> bool {
        self == BomColumn::Item
    }

    pub fn edit(self) -> CellEdit {
        match self {
            BomColumn::PartNumber
            | BomColumn::Description
            | BomColumn::Name
            | BomColumn::Revision
            | BomColumn::Vendor
            | BomColumn::Project
            | BomColumn::ProductLine
            | BomColumn::Title1
            | BomColumn::Title2
            | BomColumn::Title3 => CellEdit::Text,
            BomColumn::NotRevisionManaged => CellEdit::Check,
            BomColumn::UnitOfMeasure | BomColumn::SubassemblyBomBehavior | BomColumn::TessellationQuality => CellEdit::Choice,
            // State and Material are locked like in Properties (live 131–134).
            BomColumn::Item | BomColumn::Quantity | BomColumn::Appearance | BomColumn::State | BomColumn::Material | BomColumn::Mass => CellEdit::ReadOnly,
        }
    }

    /// Choices of a `CellEdit::Choice` column.
    pub fn choices(self) -> Vec<&'static str> {
        match self {
            BomColumn::UnitOfMeasure => UNITS_OF_MEASURE.to_vec(),
            BomColumn::SubassemblyBomBehavior => BomBehavior::ALL.iter().map(|b| b.label()).collect(),
            BomColumn::TessellationQuality => crate::partstudio::TessQuality::ALL.iter().map(|q| q.label()).collect(),
            _ => Vec::new(),
        }
    }
}

/// The BOM panel's settings, stored with the Assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BomSettings {
    #[serde(default)]
    pub bom_type: BomType,
    pub columns: Vec<BomColumn>,
    /// ⋯ › Show excluded/suppressed.
    #[serde(default)]
    pub show_excluded: bool,
    /// ⋯ › Show top level assembly row.
    #[serde(default)]
    pub show_top_level: bool,
}

impl Default for BomSettings {
    fn default() -> Self {
        BomSettings { bom_type: BomType::Flattened, columns: BomColumn::DEFAULT.to_vec(), show_excluded: false, show_top_level: false }
    }
}

impl BomSettings {
    pub fn is_default(&self) -> bool {
        *self == BomSettings::default()
    }

    /// Add column: appended at the right.
    pub fn add_column(&mut self, c: BomColumn) {
        if !self.columns.contains(&c) {
            self.columns.push(c);
        }
    }

    /// Header › Remove column (Item stays).
    pub fn remove_column(&mut self, c: BomColumn) {
        if !c.frozen() {
            self.columns.retain(|x| *x != c);
        }
    }

    /// Header › Move left (-1) / Move right (+1); Item stays first.
    pub fn move_column(&mut self, c: BomColumn, delta: i32) {
        let Some(i) = self.columns.iter().position(|x| *x == c) else { return };
        let j = i as i32 + delta;
        if c.frozen() || j < 0 || j as usize >= self.columns.len() || self.columns[j as usize].frozen() {
            return;
        }
        self.columns.swap(i, j as usize);
    }

    /// Whether Move left / Move right are possible for a column.
    pub fn can_move(&self, c: BomColumn, delta: i32) -> bool {
        let Some(i) = self.columns.iter().position(|x| *x == c) else { return false };
        let j = i as i32 + delta;
        !c.frozen() && j >= 0 && (j as usize) < self.columns.len() && !self.columns[j as usize].frozen()
    }
}

/// Onshape's "Unit of measure" choices (Properties; "Each" for a new part).
pub const UNITS_OF_MEASURE: [&str; 18] = ["", "Centimeter", "Foot", "Inch", "Meter", "Millimeter", "Yard", "Gram", "Kilogram", "Ounce", "Pound", "Liter", "Gallon", "Each", "Fluid ounce", "Milliliter", "Centiliter", "Package"];
