/** Minimal Notepad++-style menu bar with dropdowns and submenus. */

export interface MenuItem {
  label?: string;
  shortcut?: string;
  action?: () => void;
  checked?: () => boolean;
  disabled?: () => boolean;
  /** Static items or a function producing fresh items on open. */
  submenu?: MenuItem[] | (() => MenuItem[]);
  separator?: boolean;
}

export interface MenuDef {
  label: string;
  items: MenuItem[] | (() => MenuItem[]);
}

export function buildMenuBar(container: HTMLElement, menus: MenuDef[]): void {
  container.innerHTML = "";
  const openMenus: HTMLElement[] = [];

  const closeAll = (): void => {
    while (openMenus.length > 0) {
      const el = openMenus.pop();
      el?.parentElement?.classList.remove("open");
      el?.remove();
    }
  };

  const openDropdown = (menu: HTMLElement, items: MenuItem[], isSub: boolean): HTMLElement => {
    const dropdown = document.createElement("div");
    dropdown.className = "menu-dropdown";
    if (isSub) {
      dropdown.style.left = "100%";
      dropdown.style.top = "-5px";
    }
    for (const item of items) {
      if (item.separator) {
        const sep = document.createElement("div");
        sep.className = "menu-separator";
        dropdown.appendChild(sep);
        continue;
      }
      const row = document.createElement("div");
      row.className = "menu-item";
      if (item.disabled?.()) {
        row.classList.add("disabled");
      }
      const check = document.createElement("span");
      check.className = "check";
      check.textContent = item.checked?.() ? "✓" : "";
      const label = document.createElement("span");
      label.textContent = item.label ?? "";
      const shortcut = document.createElement("span");
      shortcut.className = "shortcut";
      shortcut.textContent = item.shortcut ?? (item.submenu ? "▸" : "");
      row.append(check, label, shortcut);

      if (item.submenu) {
        let subDropdown: HTMLElement | null = null;
        row.addEventListener("mouseenter", () => {
          dropdown
            .querySelectorAll(".menu-dropdown")
            .forEach((el) => el.remove());
          const subItems =
            typeof item.submenu === "function" ? item.submenu() : (item.submenu as MenuItem[]);
          subDropdown = openDropdown(row, subItems, true);
          row.appendChild(subDropdown);
        });
      } else {
        row.addEventListener("mouseenter", () => {
          dropdown.querySelectorAll(".menu-dropdown").forEach((el) => el.remove());
        });
        row.addEventListener("click", () => {
          closeAll();
          item.action?.();
        });
      }
      dropdown.appendChild(row);
    }
    menu.appendChild(dropdown);
    openMenus.push(dropdown);
    return dropdown;
  };

  for (const def of menus) {
    const menu = document.createElement("div");
    menu.className = "menu";
    const title = document.createElement("button");
    title.textContent = def.label;
    menu.appendChild(title);

    title.addEventListener("click", (e) => {
      e.stopPropagation();
      const isOpen = menu.classList.contains("open");
      closeAll();
      if (!isOpen) {
        menu.classList.add("open");
        const items = typeof def.items === "function" ? def.items() : def.items;
        openDropdown(menu, items, false);
      }
    });

    title.addEventListener("mouseenter", () => {
      if (openMenus.length > 0 && !menu.classList.contains("open")) {
        closeAll();
        menu.classList.add("open");
        const items = typeof def.items === "function" ? def.items() : def.items;
        openDropdown(menu, items, false);
      }
    });

    container.appendChild(menu);
  }

  document.addEventListener("click", (e) => {
    if (!container.contains(e.target as Node)) {
      closeAll();
    }
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closeAll();
    }
  });
}
