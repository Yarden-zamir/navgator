1. The action picker is a modal opened from the main navigator UI with Ctrl+Enter.
2. Plain Enter keeps its existing behavior and navigates to the selected target.
3. Ctrl+Enter must not change the selected result or search query before opening the picker.
4. The picker is unavailable while tag editing is active.
5. The picker is unavailable while a worktree progress overlay is active.
6. The picker closes with Esc without running an action.
7. The picker closes with Ctrl+C without running an action.
8. The picker moves selection down with Down.
9. The picker moves selection up with Up.
10. The picker moves selection down with j.
11. The picker moves selection up with k.
12. The picker runs the highlighted action with Enter.
13. The picker wraps from the last action to the first action when moving down.
14. The picker wraps from the first action to the last action when moving up.
15. The picker shows action labels exactly as configured.
16. The picker highlights one action at all times when actions exist.
17. The picker never opens with an empty action list.
18. If configuration produces no valid actions, built-in default actions are used.
19. Actions are configured under the TOML actions table.
20. The actions.defaults option controls whether built-in actions are included.
21. actions.defaults defaults to true.
22. Custom actions are configured in actions.items.
23. Custom actions are appended after built-in actions when defaults are enabled.
24. Custom actions replace built-in actions when defaults is false.
25. A custom action with an empty label is ignored.
26. A command action with an empty command is ignored.
27. An open-url action with an empty url is ignored.
28. The built-in action list begins with Navigate to.
29. Navigate to returns the same path that plain Enter would return.
30. Navigate to works from search focus.
31. Navigate to works from preview focus.
32. Navigate to works from detail focus.
33. From preview or detail focus, Navigate to uses the active preview tab path.
34. From search focus, Navigate to uses the selected entry path.
35. Project entries use the selectable worktree path when one exists.
36. Worktree entries use their worktree path.
37. Remote branch entries keep plain Enter behavior and create the worktree immediately.
38. The action picker should not be used to create remote worktrees in this version.
39. Built-in Open GitHub Desktop runs open -a GitHub Desktop {path}.
40. Built-in Open GitHub Desktop closes navgator after launching.
41. Built-in Open VS Code runs open -a Visual Studio Code {path}.
42. Built-in Open VS Code closes navgator after launching.
43. Built-in Open IntelliJ runs idea . with current_dir set to {path}.
44. Built-in Open IntelliJ closes navgator after launching.
45. Built-in Open repo online opens {github_url} with the platform opener.
46. Built-in Open repo online closes navgator after launching.
47. Built-in Open Claude session runs claude with current_dir set to {path}.
48. Built-in Open Claude session closes navgator before the session starts.
49. Built-in Open OpenCode session runs opencode with current_dir set to {path}.
50. Built-in Open OpenCode session closes navgator before the session starts.
51. Command actions have type = command.
52. Command actions require command.
53. Command actions accept args as an array of strings.
54. Command actions accept current_dir as an optional string.
55. Navigate actions have type = navigate.
56. Navigate actions do not require command, args, current_dir, or url.
57. Open URL actions have type = open-url.
58. Open URL actions require url.
59. The {path} placeholder expands to the resolved selected path.
60. The {github_url} placeholder expands to the GitHub web URL for the selected repository.
61. Placeholder expansion applies to command names.
62. Placeholder expansion applies to command args.
63. Placeholder expansion applies to command current_dir.
64. Placeholder expansion applies to open-url URLs.
65. Missing {path} prevents action execution.
66. Missing {github_url} prevents action execution.
67. GitHub URL resolution uses the selected repository origin remote.
68. GitHub URL resolution supports HTTPS GitHub remotes.
69. GitHub URL resolution supports SSH GitHub remotes.
70. GitHub URL resolution supports ssh:// GitHub remotes.
71. Non-GitHub remotes do not produce {github_url}.
72. Action execution starts only after the terminal has restored the cursor.
73. GUI launcher actions should not block the shell longer than the launcher command itself.
74. Terminal session actions may block until the launched command exits.
75. If a command fails to spawn, navgator returns an error.
76. If an opener command exits unsuccessfully, navgator returns an error.
77. If a blocking terminal command exits unsuccessfully, navgator returns an error.
78. The zsh wrapper should not cd when a non-navigate action runs.
79. The zsh wrapper should accept the line after navgator exits successfully.
80. Non-navigate actions should write no path to GATOR_OUTPUT.
81. Config schema generation includes the actions table.
82. Config schema generation includes action item variants.
83. The README documents Ctrl+Enter actions.
84. The README documents actions.defaults.
85. The README documents actions.items.
86. The README documents {path}.
87. The README documents {github_url}.
88. The help line shows Ctrl+Enter actions outside tag editing.
89. The help line does not show Ctrl+Enter actions during tag editing.
90. The picker rendering should reuse the existing rounded modal visual style.
91. The picker should fit small terminals by clamping modal width and height.
92. The picker should show a concise footer with Enter and Esc hints.
93. The picker should not disturb preview scroll state.
94. The picker should not disturb detail scroll state.
95. The picker should not trigger background provider updates itself.
96. Background provider updates may continue while the picker is open.
97. The selected action index is reset to the first action whenever the picker opens.
98. The implementation should avoid new dependencies.
99. The implementation should keep action execution code small and explicit.
100. Verification must include cargo fmt -- --check, cargo clippy --all-targets --all-features -- -D warnings, and cargo test.
