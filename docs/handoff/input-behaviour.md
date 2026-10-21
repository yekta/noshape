# Input behaviour (ui, 2026-10-21): Onshape's rules for every text field

Sources:
- Onshape's client code: `bundle-raw` (the evidence is quoted below, from the research draft);
- live: behaviours.md "Input behaviour", dom/input-*.json, input/*.png;
- qa4's inventory of our ~220 fields: /root/qa4-scratch/input-inventory.md;
- qa4's deviations: qa-round-4.md R6.

## Where each rule lives (the design: right by default, no per-field drift)

**1. Shared input: vendored gpui-base `InputState` (NOSHAPE PATCH; one-line inputs, not textareas or editors).**
- **Select on focus:** gaining the focus selects all the text, whether by a click into an unfocused field, Tab or `focus()`. A click in an already-focused field places the caret; a double-click selects a word.
  - A caller that sets a selection right after `focus()` keeps it (type-to-edit).
  - Opt out with `.select_on_focus(false)` (search boxes).
- **Blur:** the caret goes back to 0, so an unfocused field shows the start of its text.
- **Esc after an edit:** restores the text from when the field got the focus (or from the last Enter / `set_value`) and drops the focus to the view (`Window::focus_ancestor`, past the kit's frame). It is consumed, so the dialog stays open.
  - Esc without an edit passes on, so a feature dialog cancels.
  - Opt out with `.revert_on_escape(false)` (search boxes).
- **Search boxes:** `ui::ensure_placeholder` sets both opt-outs (`set_search_box`) for "Search…" / "Filter…" placeholders. States created with such a placeholder have them too.
- **Dialogs:** a gpui_kit `Dialog` without an OK handler of its own no longer closes on Enter (it lost the edits: Part / Version / Instance properties). Enter is its own action `ConfirmByKey`; a clicked OK still confirms. See gpui-base dialog.rs and gpui-component dialog.rs.

**2. Number fields: `crate::quantity_field` (pure, tested).**
- `commit_text`: on Enter / Tab / blur, `25` → `25 mm`, `5mm` → `5 mm`, `.5` → `0.5 mm`, `5.` → `5 mm`, `10+5` → `(10+5) mm`. Counts stay unitless. Text that doesn't evaluate is kept (and shown invalid).
- `step_text`: ↑ / ↓ / wheel (focused).
  - Steps: mm 1, cm 0.1, m 0.01, in 0.1, ft 0.01, yd 0.003, deg 1, rad 0.02, count 1.
  - Modifiers: Shift ×10, Ctrl ×0.1, both ×1; counts ignore them.
  - Only a plain value unchanged since its commit steps, and a step out of range does nothing.
  - The new value is committed and selected.
- Feature dialog (`feature_dialog.rs`):
  - Enter on a changed field commits and re-selects, and the dialog stays open; on an unchanged field Enter accepts the dialog.
  - The unfocused value is shown converted, with a "Depth: 32.1 mm" hover tooltip. The "= value" hint is gone (live has none).
- Next: the other number fields (sketch boxes, Variable Studio, drawing panels, assembly) move to these functions. qa4's inventory lists them; most strip units by hand today.

**3. Still per site (follow-ups):**
- the type-to-edit table cells;
- the Create variable popup for an unknown `#name`;
- the server-side "Error regenerating." cases.

## 0. Summary table

| # | Behaviour | Onshape rule | Evidence |
|---|---|---|---|
| 1a | Single click on an unfocused field | Selects **all** the text. The click does not place the caret: mouseup is `preventDefault`ed. | W @13571900 `onTextInputMouseDown/Up`; W @10022296 `onControlChangedFocus`; W @13442000 `osSelectTextWhen` |
| 1b | Click on an already-focused field | Normal caret placement at the click point (no select-all). | same: the flag is only set when `!getIsInFocus()` |
| 1c | Double-click | 1st click selects all, 2nd click is a normal browser click, then the browser's dblclick selects the word | derived from 1a/1b |
| 1d | Tab into a field | Selects all | focus → `onControlChangedFocus(true)` → `focusControlAndSelectValue()` |
| 1e | Programmatic / initial focus (dialog open, canvas box) | `focus({preventScroll})` and then `select()` | W @13570700 `setInitialFocus` |
| 1f | Selection direction (Chrome/Firefox) | Select-all is made **backward** (`setSelectionRange(0,-1,"backward")`), so a long value shows its start | W @13442000, W @2620000 |
| 1g | After blur | The caret is reset to 0 ("backward"), so the unfocused text shows its start. Unfocused text is clipped with an ellipsis. | W @13572100 `moveCursorToBeginningOfTextbox`; CSS @956547 |
| 1h | Focus swaps the text | Focus shows the **expression** (`#a*2`, `5+3`). Blur/Tab shows the **evaluated value** (`20 mm`). | W @10084525 `displayExpression/displayComputedValue` |
| 2a | Bare number | `5` → `5 mm` (the workspace default unit of the quantity type) once committed, client-side | W @10225000 `validateSimpleExpression` (fixup `b=a`) |
| 2b | Spacing / leading zero | `5mm` → `5 mm`; `.5` → `0.5 mm`; `5.` → `5 mm`; surrounding whitespace trimmed | same, regexes `valueAndUnits`, `missingLeadingZero`, `extraDecimal` |
| 2c | Unit spellings | `mm`, `millimeter`, `millimeters` (abbreviation, name, plural; `foot`→`feet`, `inch`→`inches`). Case-sensitive. The typed spelling is kept. | `getAllowedUnitStrings`, `getPluralFormOfUnit` |
| 2d | Expressions, `#var`, mixed units | Not simple, so they are sent to the server (`BTExpressionValidation`, `repairExpression:true`). The server's `updatedExpression` replaces the text, and the evaluated value is shown when unfocused. | W @10112000 `validateQuantityExpression` / `handleServerValidationResponse` |
| 2e | Display format (unfocused) | `round(v, decimals)` then JS `String()`, so **trailing zeros are dropped** (`25 mm`, `25.4 mm`). Decimals = workspace "Display decimals" per unit (default 3). Exponential if ≥1e6 or ≤1e-5. Unit text = `" " + abbreviation`. | W @11021121 `formatValue/getNumberString`; W @9648385 `pN` default 3; 957 @45880 `GB` |
| 3a | ArrowUp / ArrowDown | Steps the value and commits at once, but only when the text is a plain number(+unit), unchanged since the last commit, valid, and the autocomplete is closed | W @10104767 `getCanStepValue/stepValue` |
| 3b | Step sizes | mm 1, cm 0.1, m 0.01, in 0.1, ft 0.01, yd 0.003, deg 1, rad 0.02, unitless/other 1. The step is in the **workspace unit**, not the typed unit. | W @10105900 `getIncrementForUnit` |
| 3c | Modifiers | Shift ×10; Ctrl ×0.1 (only if the unit's display decimals ≥1); Shift+Ctrl ×1; Alt/Meta ignored; integers ignore modifiers | `stepValue` + enum `FASTER=10, SLOWER=.1` W @9435828 |
| 3d | Range | Never clamps: a step that would leave the range is a no-op. If both ±step are out of range, the step shrinks to 0.1, then 0.01. | `getIncrementWithinRange` W @10094666 |
| 3e | PageUp / PageDown | **Not** stepping. They only move through the autocomplete list when it is open. | `onKeyDown` handles only UP/DOWN/TAB; `handleAutocompleteKeyDown` |
| 3f | Mouse wheel | Steps like the arrows (same modifiers) **only while the field is focused** (listener added on focus, removed on blur). It calls `preventDefault` (no page scroll, no Ctrl-zoom). | W @13629200 AngularJS / W @4203900 Angular `wheelListener` |
| 4a | Enter, field changed or in error | Commits the field (validate → regen preview), `stopPropagation`; **the dialog stays open**. The field stays focused and is re-selected. | W @13571100 base `onKeyDown`; LIVE l.250 |
| 4b | Enter, field unchanged | Bubbles to the dialog → **OK / commit feature**. Shift+Enter → OK and open a new dialog of the same type. A held (repeat) Enter is ignored. | W @8564604 feature dialog `onKeyDown` |
| 4c | Esc, field changed | Reverts the field to the last committed value, blurs it, `stopPropagation`; **the dialog stays open** | base `onKeyDown`; LIVE l.249 |
| 4d | Esc, field unchanged | Blurs, then bubbles → **dialog Cancel** (or only closes open dropdowns first) | W @8564650 |
| 4e | Tab | Shows the evaluated value at once. The browser moves focus; blur commits. | `onKeyDown case TAB` + `onParameterFocusLost` |
| 4f | Blur (click elsewhere) | Commits if changed or in error | `onParameterFocusLost` W @13628700 |
| 5a | Invalid look | Class `os-invalid-textbox`: bg `#f9ebec`, 2 px red underline `#bd3039` (1 px border + 1 px inset shadow) | CSS @949435 / @492612; TOK |
| 5b | Invalid messages (tooltip, shown on hover, placement right) | "Enter a number or a number with units." / "Enter an integer." / "Enter a number." / "Enter a value with valid units." / "Enter a valid expression." / range: "Enter a value between __min__ and __max__.  Current value is __v__." (two spaces) / server `expressionValidationError` text | W @10113300, @10091500 |
| 5c | What is committed | Feature dialogs: **nothing**. The bad text stays in the red field and the feature keeps its last good value (`canSubmitInvalidQuantityValues:false`). Configuration and variable tables submit invalid values (the server shows the error). | W @8726587, @7152902, @14887941 |
| 6 | Focus look (dialog field) | Idle: 1 px underline `#d7d7d7`. Hover: 1 px blue `#1651b0`. Focus: 2 px blue `#1651b0` (border + inset shadow). No outline/ring. Right-aligned. | CSS @956244…969398 |
| 7a | Plain text param (`os-param-text`) | Same click/Tab select-all and the same Enter/Esc rules as numbers. Blur commits. Spaces are trimmed. Red only when **not focused**. | W @13655128 template/controller |
| 7b | Rename (inline name dialog) | On open: focus + select all. Enter/Tab = commit (trimmed); Esc = cancel; mousedown outside = commit; blur = commit after 500 ms; empty → cancel (features: allowed, resets) | W @8161900 `NameDialog` |
| 7c | Search boxes | No select-on-click. 300 ms debounce, trimmed. Esc is not consumed (it bubbles); other keys are `stopImmediatePropagation`ed on keyup. Clear ⊗ when text. | W @13440667, @2545590 |
| 8 | Differences | Canvas box: Enter **commits and closes**, Tab commits but keeps focus, an empty box is not committed on blur. Tables: click selects the cell, typing replaces, Enter commits and moves down. See §8. | W @10229628, @9807026 |

---

## 1. Focus and selection

### 1.1 The mouse-down / mouse-up trick (single click selects all)
Base controller `aA` (W @13571900; the Angular twin at W @4178900 is identical):
```js
onTextInputMouseUp(e){if(this.isRecievingFocusFromMouseEvent)return e.preventDefault(),this.isRecievingFocusFromMouseEvent=!1,!1}
onTextInputMouseDown(e){e&&this.btParameter.getIsCanvasTextBox()&&e.stopPropagation(),this.btParameter.getIsInFocus()||(this.isRecievingFocusFromMouseEvent=!0)}
onParameterFocused(e){this.btParameter.onControlChangedFocus(!0)}
```
`BTParameter.onControlChangedFocus` (W @10022296):
```js
onControlChangedFocus(e,t){e!==this.isInFocus&&(e&&this.getIsCompleted()||(this.isInFocus=e,this.isInFocus?(this.focusControlAndSelectValue(),this.onFocusSubject.next(...)):(...this.onBlurSubject.next())))}
focusControlAndSelectValue(){this.requestFocus({focusOnlyIfActive:!1,suppressSelectOnFocus:this.suppressSelectOnFocus,isReadonly:this.getIsReadOnly()})}   // W @10017500
```
The focus request is handled by the `os-select-text-when` / `osxSelectTextWhen` directive (AngularJS W @13442000, Angular W @2618682):
```js
const i=(0,a.S_)()||(0,a.nX)(),           // isBrowserFirefox || isBrowserChrome  (957 @39198 map: S_=isBrowserFirefox, nX=isBrowserChrome)
      s="text"===n.type&&!e.suppressSelectOnFocus;
s&&!i&&n.setSelectionRange(0,0,"backward"),
n!==document.activeElement&&n.focus(),
s&&(e.isReadonly?n.select():i?n.setSelectionRange(0,-1,"backward"):n.select())
```
If the input's DOM value is stale (focus has just swapped the formatted value for the expression), the directive waits for the model to update (Angular: `setTimeout`; AngularJS: a one-shot `$watch` on ngModel) and then selects.

What this means:
1. mousedown on an **unfocused** field sets `isReceivingFocusFromMouseEvent`.
2. Focus runs: the text is swapped to the expression (§1.4), then everything is selected. In Chrome/Firefox the selection is backward, so the anchor is at the end and the focus/caret is at index 0, and a long value scrolls to show its start.
3. mouseup calls `preventDefault()`, so the browser does not collapse the selection to the click point.

Net effect: **a single click selects the whole value.**

### 1.2 Click when the field is already focused
`getIsInFocus()` is true, so the flag is not set and mouseup is not prevented. Ordinary browser behaviour follows: the caret goes where you click, and dragging selects a range. The live notes agree: the caret could be placed between letters once the field had focus (LIVE l.2310–2312).

### 1.3 Double-click, Tab focus, programmatic focus
- **Double-click on an unfocused field.** The first click selects all, as in 1.1. The second mousedown hits a focused field, so the browser places the caret, and the `dblclick` then selects the word under the pointer (for `25 mm`, probably `25`). This is inferred, not coded explicitly; see the open questions.
- **Tab (keyboard focus).** This is the same `focus` → `onControlChangedFocus(true)` → select-all path, with a backward selection in Chrome.
- **Initial/programmatic focus** (`setInitialFocus`, W @13570700):
  ```js
  if(this.btParameter.getInitiallyInFocus()){const e=this.getFocusElement();e&&e.length&&(e.is(":focus")||e[0].focus({preventScroll:!0}),this.btParameter.getSuppressSelectOnFocus()||"text"===e.attr("type")&&e[0].select&&e[0].select())}else this.moveCursorToBeginningOfTextbox()
  ```
  This uses plain `select()`, a forward selection. Canvas dimension boxes use it (`r.setInitiallyInFocus()` W @10235743).
- **Re-select after commit.** `selectValueIfActive()` re-runs select-all, but only if the field is still the active element (`requestFocus({focusOnlyIfActive:true})`, W @10017400). It is called:
  - after a commit (`emitConfirmedEdit`, W @10109700);
  - after validation (`processValidationUpdate`);
  - after Esc/cancel (`cancelEditing` in BTQuantityParameter, W @10103464);
  - after an arrow step.

  So after Enter the reformatted value (for example `5 mm`) is fully selected again.
- **Manipulator drag.** `updateFromManipulator` writes the value and calls `focusControlAndSelectValue()`, so the field shows a selected value. LIVE l.236 agrees: "Depth field … updates (value selected/highlighted)".
- **Exception: sketch "telltale" typing.** When you type a digit during sketch creation, the canvas box opens with `suppressSelectOnFocus:!0` (W @8852379). The typed character stays and the caret sits after it.

### 1.4 What text is shown while focused vs unfocused
`PendingQuantity` (W @10081600…10087000):
```js
displayExpression(){this.isBoundToExpression=!0,this.populateInputWithExpression()}
displayComputedValue(e){this.isBoundToExpression=e,this.isParameterValid&&!DelayedRegen.isInDelayedRegenerationMode()?this.populateInputWithFormattedComputedValue():this.populateInputWithExpression()}
```
- On focus: `displayExpression()`, unless `displayComputedValueFirstOnFocus` is set (W @13628900 `onParameterFocused`).
- On Tab keydown and on blur: `displayComputedValue(!1)` (W @13629900 / blur handler).

So **unfocused fields show the formatted evaluated value** (e.g. `#w*2` shows as `20 mm`), and focusing reveals the raw expression. Exceptions:
- An invalid field keeps showing the expression.
- In delayed-regeneration mode the field always shows the expression.
- During an in-flight commit (`isEditCommitInProgress`) the field shows the expression.

The tooltip, when there is no error, is `"<Param name>: <last valid committed expression>"`, plus the parameter description on a new line (W @10102100 `getDisplayValueForTooltip`, W @10036700 `getTooltip`).

### 1.5 After blur
```js
onParameterFocusLost(e){setTimeout((()=>{this.btParameter.getIsInFocus()||this.btParameter.getTrackTextboxCursorPosition()||this.moveCursorToBeginningOfTextbox()}),1); ...}
moveCursorToBeginningOfTextbox(){... e&&((0,id.S_)()||(0,id.nX)())&&e.setSelectionRange(0,0,"backward")}
```
CSS (CSS @956547) for unfocused fields: `.os-param-number:not(:focus){overflow:hidden;text-overflow:ellipsis;white-space:nowrap}`. Number fields are right-aligned (`.os-param-number{text-align:right}`, CSS @959682). Text fields are left-aligned.

---

## 2. Units, parsing, formatting

### 2.1 Client-side "simple expression" validation and fixup (W @10223667, module 67332)
```js
valueAndUnits:/^\s*([-]?(?:[0-9]*\.?[0-9]+|[0-9]+\.?[0-9]*)(?:[eE][-+]?[0-9])?)\s*([a-zA-Z]*)\s*$/
numeric:     /^\s*([-]?(?:[0-9]*\.?[0-9]+|[0-9]+\.?[0-9]*)(?:[eE][-+]?[0-9])?)\s*$/
missingLeadingZero:/^\./, extraDecimal:/^(.*)\.$/
...
{const t=e.match(l.get(p).valueAndUnits);if(!t)return d.createInvalidResult();if(m=t[1],b=t[2],b&&-1===h.indexOf(b))return d.createInvalidResult();b||(g=!0,b=a)}
...
m.match(missingLeadingZero)&&(m="0"+m); const f=m.match(extraDecimal); f&&(m=f[1]);
... S=a||t===ANYTHING_WITH_UNITS?m+" "+b:m;
return {isValidBeforeFixup:!g,isValidAfterFixup:!0,fixedUpExpression:S,...}
```
Rules for length, angle and other unit-bearing types:
- The input is: optional `-`, digits with an optional decimal point, an optional exponent with **one** digit (`[eE][-+]?[0-9]`, so `1e10` is not "simple" and goes to the server), then optional whitespace and an optional unit word.
- A unit word must be one of `unit`, `plural(unit)` or `abbreviation` for that quantity type. Plural is `foot→feet`, `inch→inches`, otherwise `+s`. The match is exact and case-sensitive (`indexOf`).
- **No unit**: the default unit is the workspace unit for the quantity type (`getUnitsForQuantityType`), and its abbreviation is appended. So `5` becomes `5 mm`. LIVE l.2106 confirms: "`5` + Enter → `5 mm`".
- **Normalisation**: there is exactly one space between number and unit, surrounding whitespace is removed, `.5` becomes `0.5`, and `5.` becomes `5`. The unit keeps the spelling you typed: `5 inches` stays `5 inches`, `5mm` becomes `5 mm`.
- Unitless REAL, INTEGER and ANYTHING use the `numeric` regex and get no unit. INTEGER additionally requires `Math.floor(v)===v`. BOOLEAN accepts `true`/`false`. STRING accepts `'…'` or `"…"`.
- Decimal comma: with the "Use comma as decimal separator" setting, `,` is accepted and output uses `,` (`convertToCommaFormatIfNeeded`).
- User commits pass `repairExpression:true` (because `!e.skipRepairExpression`), so the fixed-up text is valid. Initial server values are validated with `repairExpression:false`.

### 2.2 Non-simple input: `5+3`, `2*#a`, `#w`, `1 in + 2 mm`, functions (W @10111770 `validateQuantityExpression`)
```js
const n=C.i.validateSimpleExpression(e.expression,this.getQuantityType()),i=t.repairExpression?n.isValidAfterFixup:n.isValidBeforeFixup;
if(i||this.disallowExpressions){ ...handleSimpleValidationResult... }
const a=this.buildValidationMessage(e);a.repairExpression=t.repairExpression; ... l.callAndPromise(a).then(e=>{...this.handleServerValidationResponse(a,e)...})
```
`handleServerValidationResponse`:
- On `NO_ERROR` with `updatedExpression` and repair, `setExpression(updatedExpression)`, so the text becomes whatever the server repaired it to.
- Then a range check runs on `evaluatedValue`.

The errors, in order:
1. The server's own `expressionValidationError` string.
2. `NO_UNIT` → "Enter a value with valid units."
3. Anything else → "Enter a valid expression."

Display: unfocused, mixed units are shown as the evaluated value in the workspace unit (`1 in + 2 mm` → `27.4 mm` with 3 decimals). Focused, the expression is shown as stored.

### 2.3 `#` variables and autocomplete (module 17042, W @10264900…10281500)
- **Opening keys.** `getShouldOpenAutocomplete` opens on letters, digits, Backspace, `.`, `[`, quotes, `-`, space, `#` (Shift+3) and `_` (Shift+−). The key is not consumed.
- **Filtering.** The menu filters on the identifier immediately left of the caret, regex `x = [_#a-zA-Z](?=(…)*)…`. With no identifier (e.g. you typed a digit) the menu closes at once. It is visible only when it has options.
- **Options**, sorted case-insensitively:
  - variables, shown as `#name` plus the formatted value (configuration, Variable Studio and feature variables);
  - FeatureScript functions: tolerantEquals, isInteger, abs, sqrt, log, log10, exp, floor, ceil, round, roundToPrecision, min, max, clamp, sin, cos, tan, asin, acos, atan, atan2, hypot. These are omitted when `disableUseFeaturescriptInExpressions` is set.
  - constants PI, true, false;
  - the units valid for the quantity type.
- **Typing after `#`** shows only variables, plus "New variable…" in Part Studio feature dialogs (this opens the "Create variable" popup; LIVE l.1784).
- **Highlight.** The first option is highlighted automatically.
- **Keys while open:**
  - Up/PageUp go to the previous option, Down/PageDown to the next.
  - Enter or Tab inserts the highlighted option. The key is then consumed, so it does **not** commit the field or the dialog.
  - Esc closes the menu only; it does not cancel the dialog.
  - Left/Right re-filter at the new caret position.
  - mousedown on an option inserts it.
- **Insertion.** Functions insert `name(x, y)` with the caret placed after `(`.

### 2.4 Display formatting (W @11021121 module 38956, SER @18979 module 17240, 957 @45880 module 93210)
- `formatValue(fsValue,{precisionType:UserWorkspace, showUnitsString:true})`.
- Value = base value converted to the workspace unit. Decimals = `pN(unit)`, the per-unit "Display decimals" from workspace units: `function d(e){const t=r.getValue();return t&&t.hasOwnProperty(e)?t[e]:3}` (W @9648385). The default is 3.
- Number string (`getNumberString`):
  - `r = GB(v,dec)+""` where `GB(t,e)=Math.round(t*10^e)/10^e`. JS number-to-string drops trailing zeros, so `25 mm`, not `25.000 mm`.
  - Exponential form `v.toExponential(dec)` is used if the rounded value is 0 or −0 while `|v|≥zeroTol`, or if `|v|≥1e6`, or if `|v|≤1e-5`.
  - `|v|` below the zero tolerance (1e-8 m for length, 1e-11 for angles) shows `0`.
- Unit string `U$`: `" " + abbreviation`. Powers use `²`, `³`, `⁴` or `^n`. Compound units are sorted and joined with spaces.
- Maximum precision (`togglePrecision`, by clicking the holder) only applies to **read-only** quantity fields with `supportsClickToChangePrecisionForReadonly` (e.g. measure and mass-property read-outs). It is not available on editable fields.
- Arrow-step results are written as `parseFloat((s+r).toFixed(8)) + " " + abbr`, with no rounding to display decimals.

### 2.5 When commits happen
There is **no per-keystroke commit**. `ng-change` only updates the pending expression. A commit happens on:
- Enter;
- blur (including Tab and clicking elsewhere);
- an arrow or wheel step (immediate, `skipValidation`, `continueEditingParameter`);
- choosing a variable in "Create variable".

`hasPendingValue()` compares the text against the server expression **with units required** in feature dialogs. So typing `25` over `25 mm` counts as a change and commits, which normalises it back to `25 mm`.

---

## 3. Arrow keys, Page keys, wheel

`onKeyDown` (AngularJS W @13630500; Angular W @4202208):
```js
if(this.quantityAutocomplete.getIsAutocompleteOpen())t=this.quantityAutocomplete.handleAutocompleteKeyDown(e);
else if(this.quantityAutocomplete.getShouldOpenAutocomplete(e))this.quantityAutocomplete.openAutocomplete(...);
else switch(e.which){case UP:this.btParameter.stepValue(SINGLEUP,e.shiftKey,e.ctrlKey),t=!0;break;
                     case DOWN:this.btParameter.stepValue(SINGLEDOWN,e.shiftKey,e.ctrlKey),t=!0;break;
                     case TAB:this.btParameter.pendingQuantity.displayComputedValue(!1)}
if(t)return e.stopPropagation(),e.preventDefault(),!1;
```
Up and Down are always consumed, even when no step happens, so they never move the caret to the start or end.

`getCanStepValue` (W @10104767):
```js
if(this.getIsReadOnly())return!1;if(!this.rangeChecker)return!1;if(this.isAutocompleteOpen)return!1;
if(!this.pendingQuantity.getIsExpressionValid())return!1;if(this.hasPendingValue())return!1;
return!!C.i.validateSimpleExpression(this.pendingQuantity.getExpression(),this.getQuantityType()).isValidAfterFixup
```
In words, a step requires all of these:
- the field is not read-only;
- the parameter has a range spec;
- the autocomplete is closed;
- the value is valid;
- **there is no uncommitted typing** (type `30` and then press Up, and nothing happens until you commit);
- the text is a plain number(+unit), not `#var` or `5+3`.

`stepValue`:
```js
i = QuantityType===ANYTHING ? computedFsValueUnit : this.rangeChecker.getUnits();   // workspace unit name, e.g. "millimeter"
const s=this.pendingQuantity.getNumericValue(),o=this.getIncrementForUnit(i);
let r=this.rangeChecker.getIncrementWithinRange(s,o); r*= up?1:-1;
if(INTEGER){const e=s+r; if(rangeError(e))return; set(""+e) ; confirmEditing({skipValidation:!0,editType:STANDARD_EDIT,continueEditingParameter:!0})}
else if(!isNull){let e=1;t&&n||(t?e=FASTER(10):n&&pN(i)>=1&&(e=SLOWER(.1)));r*=e;
  const o=parseFloat((s+r).toFixed(8)),a=abbr(i); l = a? o+" "+a : o+"";
  if(rangeError(o))return; l=convertToCommaFormatIfNeeded(l); setUserExpressionByIncrementing(l,o,i); confirmEditing({...continueEditingParameter:!0})}
getIncrementForUnit(e){switch(e.toLowerCase()){case MILLIMETER:return 1;case CENTIMETER:return.1;case METER:return.01;case INCH:return.1;case FOOT:return.01;case YARD:return.003;case RADIAN:return.02;case DEGREE:default:return 1}}
```
- The step works on the value **converted to the workspace unit**. In a mm workspace, a field holding `1 in` steps to `26.4 mm`.
- Out-of-range results are rejected, never clamped. `getIncrementWithinRange(e,t)`: if both `e+t` and `e−t` are out of range, retry with 0.1 (if t≥1) and then 0.01 (if t≥0.1).
- The step commits immediately, which regenerates the preview. The field stays focused and the new value is fully selected.
- **PageUp/PageDown, Home/End**: no stepping logic, so they get browser default behaviour.
- **Wheel**:
  - AngularJS: `LA(e.target).on("DOMMouseScroll mousewheel",…)` is bound in `onParameterFocused` and unbound in `onParameterFocusLost`. Its condition is `wheelDelta>0||detail<0` → up.
  - Angular: `addEventListener("wheel",this.wheelListener,{passive:!1})` on focus, where `wheelListener=e=>{e.preventDefault();if(0===e.deltaY)return;stepValue(e.deltaY<0?UP:DOWN,e.shiftKey,e.ctrlKey)}`.
  - Conditions: the field must be focused, `!getIsReadOnly()` and `getIsScrollSteppingEnabled()` (default `isScrollSteppingEnabled=!0`, W @10004586).
  - Each wheel event is one step, with the same `getCanStepValue` gate.
  - An unfocused field ignores the wheel, so the dialog/page scrolls.

---

## 4. Enter, Tab, Esc

### 4.1 Field-level (base `onKeyDown`, W @13571100; Angular W @4178100)
```js
if(this.getIsATextboxBasedControl()){
  27===e.keyCode&&this.btParameter.getIsInFocus()&&(this.btParameter.hasPendingValue()&&e.stopPropagation(),this.btParameter.cancelEditing(),this.blurTextbox()),
  13===e.keyCode&&this.btParameter.getIsInFocus()&&(this.btParameter.hasPendingValue()||this.btParameter.getHasHadAnErrorSinceLastEmittedValue())
      &&(e.stopPropagation(),this.btParameter.confirmEditing({editType:STANDARD_EDIT,isTriggeredByReturnKey:!0}))
}
```
- **Esc** always reverts (`cancelEditing` → `clearPendingValue`, which restores the server value) and blurs the field. It stops propagation only if there was a change, so:
  - Esc on a changed field: the field reverts and the dialog stays open.
  - Esc on an unchanged field: the dialog gets Esc and cancels.
  - LIVE l.249 confirms: "Esc … reverts … keeps the dialog open; a second Esc cancels the dialog".
- **Enter** with a change (or a lingering error): commits the field and stops propagation, so the dialog stays open and the field keeps focus with everything selected. With no change, it propagates to the dialog. LIVE l.250 confirms: "a second Enter commits (✓)".
- **Tab**: `displayComputedValue(false)` swaps to the evaluated value immediately. The browser then moves focus, and blur commits (`onParameterFocusLost` → `confirmEditing()` if pending or in error).

### 4.2 Feature dialog (W @8564604)
The dialog listens on `document` through the keyboard service. It is a jQuery bubble listener (957 @39198), so the field's `stopPropagation` hides keys from it.
```js
onKeyDown(e){if(e.originalEvent&&e.originalEvent.repeat)return; ... if(A(".modal-dialog").length>0)return; if(A("#name-dialog").length>0)return; ...
 if(!A(e.target).hasClass("os-canvas-text-edit")||e.which!==ENTER||!n.noModifierKeys(e))
  if(e.which===CANCEL) D.D.hasOpenedDropdowns()?D.D.closeAllDropdowns():(this.trigger("closeGenericDialog",N.CANCEL),zP(e));
  else if(!t) if(e.which===ENTER&&n.noModifierKeys(e)){ ...this.commitFeature()}
  else e.which===ENTER&&n.shiftKeyOnly(e)&&(this.model.hasParentFeature()?this.commitFeature():(zP(e),this.openNewFeatureDialogOfSameType()))}
commitFeature(){this.canSubmit()&&this.trigger("closeGenericDialog",N.OK)}
```
- A held Enter or Esc (`repeat`) is ignored.
- Esc closes open dropdowns first, otherwise it cancels the dialog.
- Enter does OK only if `canSubmit()`.
- Shift+Enter does OK and then starts the same feature again.
- Other non-feature dialogs use a pattern where Enter is ignored if the target is an INPUT or TEXTAREA (`isTextDomElement`, W @6377042, @6701779), unless the dialog sets `canCommitFromTextField`.

---

## 5. Invalid input

- **Class**: `os-invalid-textbox` on the input when `btParameter.getHasParameterError()`. Quantity fields show it **even while focused**. Text fields show it only when unfocused unless `getShowErrorsWhenFocused()`.
- **CSS** (light theme values resolved from TOK):
  - Feature dialog list item (CSS @949435): `div.os-parameter-list-item .os-param-wrapper input[type=text].os-invalid-textbox.os-param-form-item{background-color:var(--os-parameter-form-fill--error) /* #f9ebec */;border-color:var(--os-parameter-form-underline--error) /* #bd3039 */;box-shadow:inset 0 -1px 0 var(--os-error) /* #bd3039 */}`. Together with the 1 px border-bottom this gives a 2 px red underline on a pink fill. The dark theme fill is `#683232`.
  - Canvas box (CSS @493877): same fill plus `inset 0 -1px 0 #bd3039`.
  - The focus/hover blue rules all carry `:not(.os-invalid-textbox)`, so **red wins over the focus blue**.
  - Warning: `.os-warning-textbox` gives a `#fab601` underline (CSS @957733).
  - Table cell selected with an error: `.os-td.os-table-cell-selected.parameter-error:after{border:2px solid #bd3039}`.
- **Tooltip**: `getTooltip()` returns the error text first, then the warning (W @10036700). It is a Bootstrap tooltip: `data-bs-html=true`, `placement right`, `animation false`. In tables, an input with an error disables its own tooltip (`osxDisableTooltip: inTable && hasError`); the cell tooltip shows it instead (`getCellTooltip`, W @9771399).
- **Messages** (exact strings):
  - "Enter a number or a number with units." (unit-bearing type, simple parse failed, no repair path)
  - "Enter an integer." / "Enter a number."
  - "Enter a value with valid units." (number valid only after fixup without repair, or server `NO_UNIT`)
  - "Enter a valid expression." (server, generic)
  - the server's `expressionValidationError` (e.g. an unknown variable)
  - "Enter a value between __min__ and __max__.  Current value is __currentValue__." Note the **two spaces** before "Current". The bounds are formatted with 3 significant digits by `formatBounds`. LIVE l.2559 shows "Enter a value between 0 and 20.0. Current value is 50." as rendered, where HTML collapses the spaces.
  - "Enter an integer value between __min__ and __max__. Current value is __currentValue__." (for integers)
  - "Failed to validate quantity value." (danger bubble on transport failure)
- **What is committed.** For a feature dialog, nothing: `emitConfirmedEdit` only calls `super.emitConfirmedEdit` if `!getHasParameterError()` after validation (W @10109700), and feature controllers use `canSubmitInvalidQuantityValues:!1` (W @8726587). The red text stays editable, the feature keeps its previous value, and the dialog's ✓ follows `canSubmit()`. Configuration and variable tables (`canSubmitInvalidQuantityValues:!0`, W @7152902, @14887941) submit the bad expression and show the server error.

---

## 6. Focused styling (CSS @956244…969398, TOK)

| State | Rule |
|---|---|
| Base | `.os-param-number,.os-param-text{appearance:none;background:none;border:none;border-radius:0;color:var(--os-parameter-form-input-text) /* #333 */;flex:1 0 30px;height:26px;outline:none;padding:6px 3px 2px}` |
| Idle (dialog form item) | `border-bottom:1px solid var(--os-parameter-enum-border--idle)` = `#d7d7d7` |
| Hover | `border-bottom:1px solid var(--os-parameter-underline--active)` = `#1651b0` (dark: `#3e94ff`), no shadow. Hovering the label also highlights the input (`.os-param-label:hover+input`). |
| Focus / :active | `border-bottom:1px solid #1651b0; box-shadow:inset 0 -1px 0 #1651b0`, a **2 px blue underline**. No outline or focus ring. |
| Disabled | `box-shadow:none!important;cursor:not-allowed; border-bottom:1px solid var(--os-outline-tertiary)` (#d7d7d7) |
| Text selection | No custom `::selection` for inputs, so the platform default applies. LIVE l.3596 measured `rgb(10,62,168)` in the dark theme. |
| Table cell editor | Selected cell `:after{border:2px solid var(--os-table-border--focused) /* #1651b0 */}`; the input itself is borderless. |
| Search box | `:focus{border:1px solid var(--os-search-bar-outline--focused) /* #3e94ff */;box-shadow:none}` |
| Tab rename | `input.rename-tab-input{border:1px solid var(--os-active);outline:none}`, `.ng-invalid` gives a red border |
| Inline name dialog | `border-color:var(--os-textbox-outline--edit)` (#1651b0); `.has-name-error` gives `#bd3039` |

---

## 7. Plain text inputs

### 7.1 String parameters `os-param-text` (template W @13655128; controller just after)
- They use the same base controller as quantities: `ng-mousedown="$ctrl.onTextInputMouseDown()"`, `ng-mouseup="$ctrl.onTextInputMouseUp($event)"`, `os-select-text-when`. So click and Tab both select all, and a click on an already-focused field places the caret.
- The keys follow the base `onKeyDown` (§4.1): Enter commits when changed (the dialog stays open), and an unchanged field passes Enter on to dialog OK. Esc reverts and blurs, and on an unchanged field it also cancels the dialog. There is no arrow stepping or autocomplete.
- Blur commits: `onParameterFocusLost(e){super.onParameterFocusLost(e),this.btParameter.confirmEditing()}`.
- `ng-trim="{{$ctrl.getTrimSpacesNeeded()}}"`: leading and trailing spaces are trimmed unless the field tracks cursor position (annotation text).
- Validation strings: "__parameterName__ is required." and "__parameterName__ cannot contain HTML" (W @10078600).
- Error look only when unfocused: `'os-invalid-textbox': hasParameterError && (!isInFocus || showErrorsWhenFocused)`.
- `autocomplete` is set to the node id, which turns off browser autofill.

### 7.2 Rename (inline name dialog: feature/part/instance rows, document name; Vue `NameDialog`, W @8161900)
```js
const S={mounted:(e,{modifiers:t})=>{setTimeout((()=>{e.focus(),t.select&&e.select()}),0)}};   // v-focus.select → focus + select all on open
function k(){_.value?r("ok",h.value.trim()):N()}             // commit (trim) or cancel if invalid
function $(){setTimeout((()=>{M&&n.isDialogInline&&k()}),500)} // blur → commit after 500 ms (inline only)
function V(){return _.value?(r("ok",h.value.trim()),!0):!!n.isDialogInline&&(N(),!0)}
keyboard: CANCEL→N() (cancel), ENTER→V();  wrapper keydown (inline): TAB or ENTER → V() + stop/prevent; any other key except Esc → stopPropagation
document mousedown outside the wrapper → V() (commit)
input onClick / onDblclick → stopImmediatePropagation (native caret/word select)
```
- Errors: "Name is required." (empty, unless `allowEmptyName`; features allow it), "Name cannot exceed 256 characters.", "Name cannot contain HTML." In inline mode these show as a warning/danger message bubble and the input border turns red (`has-name-error`). An invalid name on Enter cancels the inline rename.
- Inside a feature dialog, the header rename (W @8559205) behaves the same way: `.trigger("focus").trigger("select")`; Enter commits, Esc cancels, blur commits. An empty name reverts unless `allowEmptyName`.
- Tab rename (`rename-tab-input`, W @12565030):
  - `bt-focus` does focus plus `select()` after a `$timeout` (W @13419811).
  - Enter and Tab submit if valid; Esc aborts; blur submits.
  - Invalid names (empty, HTML, too long) show a warning bubble: "Tab name cannot be empty." and similar.
  - Submitting an unchanged or invalid name aborts.

### 7.3 Search boxes (`os-search-box` W @13440667, `osx-search-box` W @2545590)
- There is no select-all on click; only the initial `bt-focus`/`osxFocus` selects, when `initiallyFocused`. A click places the caret normally.
- The model updates with a **300 ms debounce** and the text is trimmed. The clear ⊗ appears when the trimmed text is non-empty and empties the box immediately.
- `onSearchBoxKeyUp(e){"Escape"!==e.key&&e.stopImmediatePropagation()}`. Esc is the only key allowed to reach global handlers (to close panels and so on). Esc does **not** clear the text in the generic component.
- Enter has no special handling in the component; individual hosts may run a search on Enter (the documents page does, LIVE l.63).

---

## 8. Differences by context

| Context | Click/focus | Enter | Esc | Tab | Blur | Other |
|---|---|---|---|---|---|---|
| Feature-dialog quantity (`os-param-number dropdown-source os-param-form-item`) | select all (1.1) | changed: commit, dialog stays. Unchanged: dialog OK | changed: revert + blur. Unchanged: dialog cancel | eval display + move focus (commit on blur) | commit if changed/error | arrows/wheel step; `#` autocomplete; invalid not committed |
| On-canvas dimension / telltale box (`… os-canvas-text-edit`, W @10227500 `DimensionEditView`) | Opens with focus + select all (telltale: typed char kept, no select). Input mousedown `stopPropagation` (canvas doesn't see it); the box container's mousedown is `preventDefault`ed. | **Commit + close** in one press. `onKeyDown: CONFIRM → stopImmediatePropagation(); onOk(true)` and every confirm subscriber calls `onOk(e.isTriggeredByReturnKey)`. It closes only if there are no errors and no validation in flight. The feature dialog ignores Enter from `.os-canvas-text-edit`. | Unchanged: `onCancel()` + close (document listener, `stopImmediatePropagation`). Changed: the field handler stops propagation first, so the first Esc reverts + blurs (telltale boxes close because they subscribe to cancel). See open questions. | `confirmEditing()` + `preventDefault` (**focus stays**) except for tolerant params; shows the evaluated value | Commits only if the text is non-empty (`!boundDisplayValue.trim()`, so an empty box is not committed) | 100 px wide, bottom border only; error CSS @493877 |
| Table cell editor (variable table, configurations, BOM; `os-param-number`/`os-param-text` without `os-param-form-item`) | Click selects the **cell** (2 px blue frame). Editing starts on double-click, on Enter/Space, on click inside the cell "window", or by **typing a printable key** (digits, letters, numpad, `.`, `+-*/`, Backspace). The key reveals the editor with select-all, so the typed key replaces the content. (W @9770630, @9809023, @10047234) | Commit, then the selection moves **down** (Shift+Enter: up) | Cancel; the same cell stays selected (editor closed) | Commit; next cell (Shift+Tab: previous) | commit | Arrow keys on a selected (non-editing) cell move the selection. While editing, Up/Down still step quantities. Invalid values **are** submitted in configuration/variable tables. After commit there is no re-select-all in tables (`getIsInTable()` skips `selectValueIfActive`). |
| Rename (inline name dialog) | focus + select all on open | commit (trim) | cancel | commit | commit after 500 ms (and mousedown outside commits) | message-bubble errors |
| Plain text param | select all | as quantity | as quantity | move focus | commit | trims, no stepping |
| Search box | caret at click | host-defined | bubbles (not cleared) | normal | nothing | 300 ms debounce, ⊗ clear |

---

## 9. Open questions for a live test

1. **Double-click** on an unfocused `25 mm`: is the result word selection (`25`), or does it stay select-all? The mouseup of the first click is prevented, so the second click's behaviour is the browser's.
2. **Server-repaired expressions.** What `updatedExpression` comes back for `5+3`, `2*3`, `#w*2` and `1 in + 2 mm` in a length field? Are units appended (`(5+3) mm`?), or does the server report NO_UNIT ("Enter a value with valid units.")? What does the field show focused vs unfocused afterwards?
3. **Unfocused display of a variable expression.** Confirm that a committed `#w` shows the evaluated value (`10 mm`) when unfocused and `#w` when focused, and that the tooltip reads `Depth: #w`.
4. **Exponent formatting** for very large or small values: is `1.000e+6 mm` printed exactly like that?
5. **Wheel with Shift.** In Chrome on Linux, Shift+wheel produces horizontal deltas (`deltaY=0`), and the Angular listener returns early. Does Shift+wheel step at all, and does Ctrl+wheel step or zoom?
6. **Arrow step on a value typed in a non-workspace unit** (e.g. `1 in` in a mm document): confirm the result is `26.4 mm`.
7. **Canvas dimension box with a changed value, first Esc**: does it only revert, or also close? Check the sketch dimension box and the sketch telltale separately.
8. **Empty feature-dialog field + Enter or blur**: which error text appears ("Enter a valid expression."?), and is the field red?
9. **Selection colour** of selected input text in the light theme (platform default; dark measured `rgb(10,62,168)`).
10. Which parameter has **initial focus** for each feature type? Extrude focuses the selection list (LIVE l.239). Does any dialog open with a numeric field focused and selected?
11. **Shift+Enter in a field with a pending change**: does it commit only (the field handler stops propagation), so a second Shift+Enter is needed to repeat the feature?
12. **Alt+Arrow and Meta+Arrow** in a number field: confirm they still step by ×1. The code ignores alt and meta.

## Status (21 Oct, ui)

**Verified in the app** (x-r4b; x-drw):
- **Depth field:**
  - a click selects all;
  - ↑ steps 12 → 13 mm, and Shift+↑ +10;
  - `10+5` + Enter shows `(10+5) mm`, selected, with the dialog open;
  - Esc after an edit reverts, the underline goes and typing goes nowhere; a second Esc cancels.
- **Variable Studio cells:** Esc in the Name cell reverts; Esc in the Value cell reverts and doesn't commit.
- **Search boxes:** a click doesn't select all; Esc keeps the text.
- **qa4 F13:** Esc in an open select's search box only closes the list.
- **qa4 F22:** the tooltip hides after a click.

**Kit pieces:**
- `InputState::{select_on_focus, revert_on_escape, set_search_box, revert_escape, set_frame_focus}`;
- `Window::focus_parent_of`;
- the dialog's Enter no longer confirms without an OK handler (`enter_confirms`).

**App pieces:**
- `quantity_field::{commit_text, step_text, invalid_field}`;
- the feature dialog's `commit_number` / `step_number`;
- the dimension box's `step_dimension`.

**Open:**
- Enter / Name focus in Part › Properties (item 9) is built, still to check.
- Feature-rename Esc, still to check.
- Migrate the remaining number fields to `quantity_field`: drawing panels, assembly and the sketch box commit
  (psb).
- qa4 F17 (dropdown at 900 px) and F24 (Workspace units over the sketch) wait for the live request "ui (10-21 qa4
  core-workflow F17/F24)".
