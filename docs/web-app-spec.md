# Web App Spec

## Server

1. `navgator web` serves the navigator on `http://127.0.0.1:<port>`. The default port is 4747.
2. The server listens on `127.0.0.1` only.
3. The server rejects a request whose `Host` header is not this server.
4. The server rejects a non-GET request whose `Origin` is not this server.
5. The server sends no CORS headers.
6. If a navgator server answers on the port, `navgator web` opens its page and exits.
7. If another program uses the port, `navgator web` fails with an error.
8. `--no-open` starts or finds the server and does not open a browser.
9. The server loads config like the terminal UI, including `--config-entry` values.
10. The server reads the same caches, tags and usage history as the terminal UI.

## Navigator

11. The page has the same results, preview, detail tabs, action picker, create flow, remote branches and sort modes as the terminal UI.
12. The page resolves key presses with the configured keymap.
13. A `navigate` binding opens the action picker. Actions of type `navigate` do not show.
14. Actions run without a terminal. Stdin is closed and the server captures the output.
15. The page reloads results when the server reports a change.

## Tags

16. A tag has the same color as in the terminal UI: a hue from the FNV-1a hash of the tag, at 60% saturation and 55% lightness.
17. A click on a tag adds `#tag` to the search query. When the query has `#tag`, the click removes it.
18. The preview panel shows the tags of the selected result and an "Edit tags" button.
19. The tag editor has a remove button on each tag and shows known tags that the input matches as clickable suggestions.

## Settings

20. The settings page opens at `#settings` and from the "Settings" button in the footer.
21. The settings page edits `$NAVGATOR_CONFIG` when it is set, else the default user config path.
22. The page lists the other config files that navgator loads. It does not edit them.
23. The form comes from the config JSON schema. Titles, descriptions, enums and required keys come from the schema.
24. "Reset" removes a key from the file, so the default applies.
25. A form save changes only the values that changed. Comments and the layout of other values stay.
26. A list of tables keeps the comments of an item when the item moves or another item goes.
27. The TOML view saves the file text as typed.
28. After a save, the server reloads the config. If the new config does not load, the server puts the old file back and shows the load error.
29. Path values complete from the file system. Keybinding targets complete from the actions that the context accepts. Picker entries complete from the action ids.
30. A keybinding chord can be typed, or recorded from the next key press.
31. Each keybinding context shows its built-in bindings. A click on one fills the chord to override it.
32. The navigator loads the config again when the settings page closes.

## Web Additions

33. The page bundles the Symbols Nerd Font for the icon code points only, so icons show in every font and on machines without a Nerd Font.
34. A double click, a right-click or the "⋯" button on a row opens the action picker for that row.
35. The results toolbar selects the sort mode and turns remote branches on or off.
36. The preview header copies the path, opens the GitHub page of the `origin` remote, and opens the action picker.
37. A drag on the line between two panels resizes them. The page keeps the sizes in local storage. A double click on the line restores the default size.
38. A click on a footer key hint runs the same action as the key.
39. F1 or the "Keys" button lists every binding of every context.
40. A click on the backdrop or the × button closes an overlay. A running task cannot close.
41. Notices show as toasts. A click dismisses a toast.
42. The page title names the selected result.
