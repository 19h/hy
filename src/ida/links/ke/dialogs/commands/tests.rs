use super::*;

#[test]
fn script_dialogs_pass_literal_text_through_environment_variables() {
    let text = "'quoted\" <span>& $(); `n\nnext line";
    for platform in [Platform::Mac, Platform::Windows] {
        let confirm = confirm_commands(platform, text).remove(0);
        for (command, key) in [
            (confirm, "KE_DLG_MSG"),
            (error_command(platform, text), "KE_DLG_MSG"),
            (progress_command(platform, text), "KE_DLG_FILE"),
        ] {
            assert!(command.get_args().all(|arg| !arg.to_string_lossy().contains(text)));
            assert!(
                command.get_envs().any(|(name, value)| name == key && value == Some(text.as_ref()))
            );
        }
    }
}

#[test]
fn linux_confirmation_escapes_markup_and_keeps_tool_precedence() {
    let commands = confirm_commands(Platform::Linux, "<span>&</span>");
    assert_eq!(commands[0].get_program(), "zenity");
    assert_eq!(commands[1].get_program(), "kdialog");
    let args: Vec<_> = commands[0].get_args().map(|arg| arg.to_string_lossy()).collect();
    assert_eq!(args, ["--question", "--title=KE", "--text=&lt;span&gt;&amp;&lt;/span&gt;"]);
    let args: Vec<_> = commands[1].get_args().map(|arg| arg.to_string_lossy()).collect();
    assert_eq!(args, ["--yesno", "&lt;span&gt;&amp;&lt;/span&gt;", "--title", "KE"]);
    for command in [
        error_command(Platform::Linux, "literal <text>"),
        progress_command(Platform::Linux, "literal <text>"),
    ] {
        assert_eq!(command.get_program(), "notify-send");
        assert!(command.get_args().any(|arg| arg.to_string_lossy().contains("literal <text>")));
    }
}
