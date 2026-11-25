# gpui-kit notes

Pitfalls met while building Latexo on gpui-kit 0.10.0. Postino's `docs/gpui-notes.md` has the
older ones shared by every app.

- **`use gpui_kit::*` shadows `#[test]`.** The glob brings gpui's `test` attribute macro into
  scope, and a test module with `use super::*` in a view file fails with "recursion limit reached
  while expanding `#[test]`". Import what the tests need by name.
- **sysinfo's `Gpus` is not `Send`** (NVML and Vulkan handles), so the `Sampler` cannot move into
  a task of the background executor. It lives on a thread of its own and sends samples through
  an `async-channel` (`views/root.rs`).
- **`flex_grow` takes the factor** (`flex_grow(2.0)`), and there is no `flex_2`. Proportional
  columns are `flex_grow(n).flex_basis(relative(0.0))`.
- **A `uniform_list` row must set `w_full()`**, or each row is as wide as its content and the
  columns of a table do not line up with the header.
- **`dropdown_menu` on a telaio `MenuTrigger`** needs `gpui_kit::component::menu::DropdownMenu`
  in scope, and `context_menu` on a div needs `ContextMenuExt`.
