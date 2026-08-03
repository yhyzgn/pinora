//! Overlay 键盘与鼠标输入的纯意图判定。
//!
//! 调用方负责读取事件、修改标注文档、提交任务和控制窗口；本模块只将既有输入规则
//! 映射为稳定意图，避免把这些规则绑定到 EventLoop 或窗口宿主。

use pinora_core::AnnotateTool;
use winit::keyboard::ModifiersState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationHistoryAction {
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEnterAction {
    InsertLineBreak,
    Commit,
}

pub fn annotation_history_action(
    control_pressed: bool,
    shift_pressed: bool,
    character: &str,
) -> Option<AnnotationHistoryAction> {
    if !control_pressed {
        return None;
    }
    match character {
        "z" | "Z" if shift_pressed => Some(AnnotationHistoryAction::Redo),
        "z" | "Z" => Some(AnnotationHistoryAction::Undo),
        "y" | "Y" => Some(AnnotationHistoryAction::Redo),
        _ => None,
    }
}

pub fn text_enter_action(modifiers: ModifiersState, text_editing: bool) -> Option<TextEnterAction> {
    if !text_editing {
        return None;
    }
    if modifiers.shift_key() && !modifiers.control_key() {
        Some(TextEnterAction::InsertLineBreak)
    } else {
        Some(TextEnterAction::Commit)
    }
}

pub fn annotation_nudge_step(modifiers: ModifiersState) -> i32 {
    if modifiers.shift_key() { 10 } else { 1 }
}

pub const fn overlay_click_finishes_copy(tool: AnnotateTool, is_double_click: bool) -> bool {
    is_double_click && !matches!(tool, AnnotateTool::Number | AnnotateTool::Select)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_shortcuts_distinguish_undo_and_redo() {
        assert_eq!(
            annotation_history_action(true, false, "z"),
            Some(AnnotationHistoryAction::Undo)
        );
        assert_eq!(
            annotation_history_action(true, true, "Z"),
            Some(AnnotationHistoryAction::Redo)
        );
        assert_eq!(
            annotation_history_action(true, false, "y"),
            Some(AnnotationHistoryAction::Redo)
        );
        assert_eq!(annotation_history_action(false, false, "z"), None);
    }

    #[test]
    fn text_enter_distinguishes_multiline_input_from_commit() {
        assert_eq!(
            text_enter_action(ModifiersState::SHIFT, true),
            Some(TextEnterAction::InsertLineBreak)
        );
        assert_eq!(
            text_enter_action(ModifiersState::CONTROL | ModifiersState::SHIFT, true),
            Some(TextEnterAction::Commit)
        );
        assert_eq!(
            text_enter_action(ModifiersState::empty(), true),
            Some(TextEnterAction::Commit)
        );
        assert_eq!(text_enter_action(ModifiersState::SHIFT, false), None);
    }

    #[test]
    fn nudge_step_is_one_or_ten_pixels() {
        assert_eq!(annotation_nudge_step(ModifiersState::empty()), 1);
        assert_eq!(annotation_nudge_step(ModifiersState::SHIFT), 10);
    }

    #[test]
    fn double_click_copy_exempts_sequence_and_selection_tools() {
        assert!(overlay_click_finishes_copy(AnnotateTool::Rect, true));
        assert!(!overlay_click_finishes_copy(AnnotateTool::Rect, false));
        assert!(!overlay_click_finishes_copy(AnnotateTool::Number, true));
        assert!(!overlay_click_finishes_copy(AnnotateTool::Select, true));
    }
}
