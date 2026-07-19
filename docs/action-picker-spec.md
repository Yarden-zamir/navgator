1. The action picker is a modal opened from the main navigator UI with an `actions` binding.
2. Key names in this spec describe the runtime default keymap and may be remapped by configuration.
3. The default navigator keymap binds plain Enter to navigate to the selected target.
4. An `actions` binding must not change the selected result or search query before opening the picker.
5. The picker is unavailable while tag editing is active.
6. The picker is unavailable while a worktree progress overlay is active.
7. The picker closes with Esc without running an action.
8. The picker closes with Ctrl+C without running an action.
9. The picker moves selection down with Down.
10. The picker moves selection up with Up.
11. The picker moves selection down with j.
12. The picker moves selection up with k.
13. The picker runs the highlighted action with Enter and keeps the parent shell session open.
14. The picker filters actions when the user types a search query.
15. Backspace removes one character from the action search query.
16. The picker wraps from the last visible action to the first visible action when moving down.
17. The picker wraps from the first visible action to the last visible action when moving up.
18. The picker shows action labels exactly as configured.
19. The picker may show configured icons before labels.
20. The picker highlights one visible action when visible actions exist.
21. The picker can show an empty-state message when no actions match search or file conditions.
22. If configuration produces no valid actions, built-in default actions are used.
23. Optional `actions.picker` limits visible actions by stable action ID and controls their picker order.
24. Unknown or duplicate picker IDs fail config loading.
25. An empty picker allowlist shows the existing empty state.
26. Picker filtering does not remove actions from direct keybinding resolution.
27. Navigator bindings for `actions` default to Ctrl+Enter and Ctrl+Space.
28. The UI shows the first resolved binding for relevant picker actions.
29. Actions are configured under the TOML actions table.
30. Listed actions replace built-ins by default.
31. include_defaults = true prepends built-ins before listed actions.
32. Custom actions are configured in actions.items.
33. Custom actions are appended after built-in actions when include_defaults is enabled.
34. Custom actions replace built-in actions when include_defaults is omitted or false.
35. A custom action with an empty label is ignored.
36. A command action with an empty command is ignored.
37. An open-url action with an empty url is ignored.
38. The built-in action list begins with Navigate to.
39. Navigate to returns the same resolved target path as the core `navigate` action.
40. Navigate to works from search focus.
41. Navigate to works from preview focus.
42. Navigate to works from detail focus.
43. From preview or detail focus, Navigate to uses the active preview tab path.
44. From search focus, Navigate to uses the selected entry path.
45. Project entries use the selectable worktree path when one exists.
46. Worktree entries use their worktree path.
47. A target-dependent action on a remote branch prepares the configured checkout or worktree first.
48. The `actions` binding opens the picker for the resulting local path after branch preparation.
49. Built-in Open GitHub Desktop runs open -a GitHub Desktop {path}.
50. Built-in Open GitHub Desktop closes navgator after launching.
51. Built-in Open VS Code runs open -a Visual Studio Code {path}.
52. Built-in Open VS Code closes navgator after launching.
53. Built-in Open IntelliJ runs idea . with current_dir set to {path}.
54. Built-in Open IntelliJ closes navgator after launching.
55. Built-in Open repo online opens {github_url} with the platform opener.
56. Built-in Open repo online closes navgator after launching.
57. Built-in Open Claude session runs claude with current_dir set to {path}.
58. Built-in Open Claude session closes navgator before the session starts.
59. Built-in Open OpenCode session runs opencode with current_dir set to {path}.
60. Built-in Open OpenCode session closes navgator before the session starts.
61. Command actions have type = command.
62. Command actions require command.
63. Command actions accept args as an array of strings.
64. Command actions accept current_dir as an optional string.
65. Navigate actions have type = navigate.
66. Navigate actions do not require command, args, current_dir, or url.
67. Open URL actions have type = open-url.
68. Open URL actions require url.
69. The {path} placeholder expands to the resolved selected path.
70. The {github_url} placeholder expands to the GitHub web URL for the selected repository.
71. Placeholder expansion applies to command names.
72. Placeholder expansion applies to command args.
73. Placeholder expansion applies to command current_dir.
74. Placeholder expansion applies to open-url URLs.
75. Missing {path} prevents action execution.
76. Missing {github_url} prevents action execution.
77. GitHub URL resolution uses the selected repository origin remote.
78. GitHub URL resolution supports HTTPS GitHub remotes.
79. GitHub URL resolution supports SSH GitHub remotes.
80. GitHub URL resolution supports ssh:// GitHub remotes.
81. Non-GitHub remotes do not produce {github_url}.
82. The picker runs the highlighted action with a `run-and-close` binding and asks the shell wrapper to close the parent shell session after the action succeeds.
83. Action icons support Nerd Font glyphs and emoji.
84. Action file_condition is resolved relative to the selected target path when it is relative.
85. Actions with unmet file_condition are hidden from the picker.
86. `navgator actions` opens directly to the action picker for the first result.
87. `navgator actions <path>` opens directly to the action picker for the provided path.
88. GUI launcher actions should not block the shell longer than the launcher command itself.
89. Terminal session actions may block until the launched command exits.
90. If a command fails to spawn, navgator returns an error.
91. If an opener command exits unsuccessfully, navgator returns an error.
92. If a blocking terminal command exits unsuccessfully, navgator returns an error.
93. The zsh wrapper should not cd when a non-navigate action runs.
94. The zsh wrapper should accept the line after navgator exits successfully.
95. Non-navigate actions should write no path to GATOR_OUTPUT unless a picker binding requested a close-session marker.
96. Config schema generation includes the actions table.
97. Config schema generation includes action item variants.
98. The README documents context-specific action picker bindings.
99. The README documents actions.include_defaults.
100. The README documents actions.items.
101. The README documents {path}.
102. The README documents {github_url}.
103. The help line shows the first configured picker binding outside tag editing.
104. The help line does not show the `actions` binding during tag editing.
105. The picker rendering should reuse the existing rounded modal visual style.
106. The picker should fit small terminals by clamping modal width and height.
107. The picker should show a concise footer with Enter and Esc hints.
108. The picker should not disturb preview scroll state.
109. The picker should not disturb detail scroll state.
110. The picker should not trigger background provider updates itself.
111. Background provider updates may continue while the picker is open.
112. The selected action index is reset to the first action whenever the picker opens.
113. The implementation should avoid new dependencies.
114. The implementation should keep action execution code small and explicit.
115. Verification must include cargo fmt -- --check, cargo clippy --all-targets --all-features -- -D warnings, and cargo test.
