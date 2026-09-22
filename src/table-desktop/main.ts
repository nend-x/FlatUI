/* =========================================================================
   TABLE 5 — desktop table. The desktop icons in a medium movable window
   (position persists to tables.json). Same icon tiles as the old flatlight
   grid: click launches, shift+click runs as admin, right-click gives the
   Open / Rename / Delete menu, right-click on empty space gives
   New Folder / New File / Refresh.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme, type ThemePayload } from "../shared/theme";

interface LauncherItem {
  id: string;
  name: string;
  path: string;
  icon_data_url: string | null;
  is_folder: boolean;
}

const root = document.getElementById("mt-root")!;
const grid = document.getElementById("td-grid")!;

// Theme + icon-recolor values are applied together by the shared applyTheme
// — startup load AND live theme://changed events both go through it.

listen<ThemePayload>("theme://changed", (e) => {
  applyTheme(e.payload);
});

listen<boolean>("icon-recolor://changed", (e) => {
  document.documentElement.classList.toggle("icon-recolor", e.payload);
});

// ===== Show / hide =====
let closing = false;

listen("table://desktop-shown", () => {
  closing = false;
  root.classList.remove("shown");
  void root.offsetWidth;
  root.classList.add("shown");
  void refresh();
});

function close() {
  if (closing) return;
  closing = true;
  if (launchCloseTimer !== null) {
    window.clearTimeout(launchCloseTimer);
    launchCloseTimer = null;
  }
  root.classList.remove("shown");
  // Let the pop-out play before the backend hides the window.
  setTimeout(() => invoke("close_table", { name: "desktop" }), 240);
}

document.getElementById("mt-close")!.addEventListener("click", close);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") close();
});

// ===== Desktop items =====
let items: LauncherItem[] = [];

async function refresh() {
  try {
    items = await invoke<LauncherItem[]>("get_desktop_items");
  } catch {
    items = [];
  }
  render();
}

listen<LauncherItem[]>("launcher://items-updated", (e) => {
  items = e.payload;
  render();
});

function launch(item: LauncherItem, source?: HTMLElement | null) {
  invoke("launch_desktop_item", { itemId: item.id });

  // Launch feedback: the opened icon spins for 3s, then the desktop
  // table closes itself (the opened app/folder takes over from here).
  if (source) {
    const iconBox = source.querySelector<HTMLElement>(".icon-box");
    if (iconBox && !iconBox.classList.contains("spinning")) {
      iconBox.classList.add("spinning");
    }
  }
  closeAfterLaunch();
}

let launchCloseTimer: number | null = null;
function closeAfterLaunch() {
  if (launchCloseTimer !== null) window.clearTimeout(launchCloseTimer);
  // Let the 3s spin animation play before the table dismisses.
  launchCloseTimer = window.setTimeout(close, 3000);
}

function render() {
  grid.innerHTML = "";
  for (const item of items) {
    const el = document.createElement("div");
    el.className = "launcher-item";
    el.dataset.id = item.id;
    el.title = item.name;

    const iconBox = document.createElement("div");
    iconBox.className = "icon-box";
    if (item.icon_data_url) {
      const img = document.createElement("img");
      img.src = item.icon_data_url;
      img.alt = item.name;
      img.draggable = false;
      iconBox.appendChild(img);
    } else {
      const fb = document.createElement("span");
      fb.textContent = item.is_folder ? "▤" : (item.name || "?")[0].toUpperCase();
      fb.style.cssText = "font-family:var(--font-display);font-size:20px;color:var(--sand);";
      iconBox.appendChild(fb);
    }

    const label = document.createElement("div");
    label.className = "label";
    label.textContent = item.name;

    el.appendChild(iconBox);
    el.appendChild(label);

    el.addEventListener("click", (e) => {
      if (e.shiftKey) {
        invoke("execute_run_admin", { command: item.path });
      } else {
        launch(item, el);
      }
    });

    el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      e.stopPropagation();
      showItemContextMenu(e.clientX, e.clientY, item);
    });

    grid.appendChild(el);
  }

  if (items.length === 0) {
    const empty = document.createElement("div");
    empty.className = "td-empty";
    empty.textContent = "Desktop folder is empty";
    grid.appendChild(empty);
  }
}

// ===== Context menus =====
grid.addEventListener("contextmenu", (e) => {
  if (e.target === grid) {
    e.preventDefault();
    showBackgroundContextMenu(e.clientX, e.clientY);
  }
});

function showItemContextMenu(x: number, y: number, item: LauncherItem) {
  document.querySelectorAll(".context-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  const open = document.createElement("div");
  open.className = "context-menu-item";
  open.textContent = "Open";
  open.addEventListener("click", () => {
    menu.remove();
    launch(item, document.querySelector<HTMLElement>(`.launcher-item[data-id="${CSS.escape(item.id)}"]`));
  });
  menu.appendChild(open);

  const sep1 = document.createElement("div");
  sep1.className = "context-menu-separator";
  menu.appendChild(sep1);

  const rename = document.createElement("div");
  rename.className = "context-menu-item";
  rename.textContent = "Rename";
  rename.addEventListener("click", () => {
    menu.remove();
    showRenameDialog(item);
  });
  menu.appendChild(rename);

  const del = document.createElement("div");
  del.className = "context-menu-item danger";
  del.textContent = "Delete";
  del.addEventListener("click", () => {
    menu.remove();
    invoke("delete_desktop_item", { itemId: item.id });
  });
  menu.appendChild(del);

  document.body.appendChild(menu);
  clampMenu(menu, x, y);
}

function showBackgroundContextMenu(x: number, y: number) {
  document.querySelectorAll(".context-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  const newItem = document.createElement("div");
  newItem.className = "context-menu-item";
  newItem.innerHTML = `<span>New</span><span class="context-menu-submenu-arrow">▸</span>`;

  const submenu = document.createElement("div");
  submenu.className = "context-menu-submenu";

  const folderItem = document.createElement("div");
  folderItem.className = "context-menu-item";
  folderItem.textContent = "Folder";
  folderItem.addEventListener("click", () => {
    menu.remove();
    showDialog("New folder", "", "Folder name", (name) => {
      invoke("create_desktop_item", { name, isFolder: true });
    });
  });

  const fileItem = document.createElement("div");
  fileItem.className = "context-menu-item";
  fileItem.textContent = "File";
  fileItem.addEventListener("click", () => {
    menu.remove();
    showDialog("New file", "", "name.extension", (name) => {
      invoke("create_desktop_item", { name, isFolder: false });
    });
  });

  submenu.appendChild(folderItem);
  submenu.appendChild(fileItem);
  newItem.appendChild(submenu);
  menu.appendChild(newItem);

  const sep = document.createElement("div");
  sep.className = "context-menu-separator";
  menu.appendChild(sep);

  const refreshItem = document.createElement("div");
  refreshItem.className = "context-menu-item";
  refreshItem.textContent = "Refresh";
  refreshItem.addEventListener("click", () => {
    menu.remove();
    invoke("refresh_desktop");
  });
  menu.appendChild(refreshItem);

  document.body.appendChild(menu);
  clampMenu(menu, x, y);
}

function clampMenu(menu: HTMLElement, x: number, y: number) {
  const rect = menu.getBoundingClientRect();
  if (rect.right > window.innerWidth - 8) menu.style.left = `${x - rect.width}px`;
  if (rect.bottom > window.innerHeight - 8) menu.style.top = `${y - rect.height}px`;
}

document.addEventListener("mousedown", (e) => {
  document.querySelectorAll(".context-menu").forEach((el) => {
    if (!el.contains(e.target as Node)) el.remove();
  });
});

// ===== Rename dialog (same as the launcher's modal) =====
function showRenameDialog(item: LauncherItem) {
  showDialog("Rename", item.name, "New name", (name) => {
    invoke("rename_desktop_item", { itemId: item.id, newName: name });
  });
}

function showDialog(
  title: string,
  initialValue: string,
  placeholder: string,
  onConfirm: (name: string) => void
) {
  document.querySelectorAll(".modal-backdrop").forEach((el) => el.remove());

  const backdrop = document.createElement("div");
  backdrop.className = "modal-backdrop";

  const dialog = document.createElement("div");
  dialog.className = "modal-dialog";

  const titleEl = document.createElement("div");
  titleEl.className = "modal-title";
  titleEl.textContent = title;

  const input = document.createElement("input");
  input.className = "modal-input";
  input.type = "text";
  input.value = initialValue;
  input.placeholder = placeholder;

  const actions = document.createElement("div");
  actions.className = "modal-actions";

  const cancel = document.createElement("button");
  cancel.className = "modal-btn";
  cancel.textContent = "Cancel";

  const confirm = document.createElement("button");
  confirm.className = "modal-btn primary";
  confirm.textContent = "OK";

  actions.appendChild(cancel);
  actions.appendChild(confirm);
  dialog.appendChild(titleEl);
  dialog.appendChild(input);
  dialog.appendChild(actions);
  backdrop.appendChild(dialog);
  document.body.appendChild(backdrop);

  setTimeout(() => input.focus(), 10);

  const closeDialog = () => backdrop.remove();
  cancel.addEventListener("click", closeDialog);
  backdrop.addEventListener("click", (e) => {
    if (e.target === backdrop) closeDialog();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      const v = input.value.trim();
      if (v) {
        onConfirm(v);
        closeDialog();
      }
    } else if (e.key === "Escape") {
      closeDialog();
    }
  });
  confirm.addEventListener("click", () => {
    const v = input.value.trim();
    if (v) {
      onConfirm(v);
      closeDialog();
    }
  });
}

// ===== Init =====
(async function init() {
  try {
    const theme = await invoke<ThemePayload | null>("get_active_theme");
    if (theme) applyTheme(theme);
    if (await invoke<boolean>("load_icon_recolor").catch(() => false)) {
      document.documentElement.classList.add("icon-recolor");
    }
  } catch {}
  await refresh();
})();
