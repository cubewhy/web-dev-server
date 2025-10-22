(() => {
  const configKey = "__WEB_DEV_SERVER_CONFIG__";
  const config = window[configKey] || {};
  const wsPath = typeof config.wsPath === "string" ? config.wsPath : "/_live/ws";
  const diffMode = Boolean(config.diffMode);

  let retryDelay = 500;

  const log = (...parts) => console.log("[web-dev-server]", ...parts);
  const error = (...parts) => console.error("[web-dev-server]", ...parts);

  function connect() {
    const protocol = window.location.protocol === "https:" ? "wss" : "ws";
    const target = `${protocol}://${window.location.host}${wsPath}`;
    const socket = new WebSocket(target);

    socket.addEventListener("open", () => {
      log("connected");
      retryDelay = 500;
    });

    socket.addEventListener("message", (event) => {
      try {
        const message = JSON.parse(event.data);
        handleMessage(message);
      } catch (err) {
        error("received malformed message", err);
      }
    });

    socket.addEventListener("close", () => {
      retryDelay = Math.min(retryDelay * 2, 8000);
      setTimeout(connect, retryDelay);
    });

    socket.addEventListener("error", () => {
      socket.close();
    });
  }

  function handleMessage(message) {
    if (!message || typeof message.type !== "string") {
      return;
    }

    switch (message.type) {
      case "reload":
        hardReload(window.location.pathname);
        break;
      case "diff":
        if (!diffMode) {
          hardReload(window.location.pathname);
          return;
        }
        if (message.resource === "html") {
          if (!pathsMatch(message.path, window.location.pathname)) {
            log(
              "skipping HTML diff for non-matching path",
              message.path,
              window.location.pathname
            );
            return;
          }
          applyHtmlDiff(message.path);
        } else if (message.resource === "css") {
          applyCssDiff(message.path);
        } else {
          hardReload(window.location.pathname);
        }
        break;
      default:
        break;
    }
  }

  async function applyHtmlDiff(path) {
    if (!path) {
      hardReload(window.location.pathname);
      return;
    }

    try {
      const response = await fetch(cacheBustUrl(path), { cache: "no-store" });
      const text = await response.text();
      const parser = new DOMParser();
      const doc = parser.parseFromString(text, "text/html");

      if (!doc) {
        throw new Error("failed to parse HTML");
      }

      updateTitle(doc);
      mergeHead(doc.head);
      replaceBody(doc.body);
      reactivateScripts(document.body);
      reactivateScripts(document.head);
    } catch (err) {
      error("failed to apply HTML diff", err);
      hardReload(window.location.pathname);
    }
  }

  function updateTitle(doc) {
    if (doc.title) {
      document.title = doc.title;
    }
  }

  function replaceBody(newBody) {
    if (!newBody) {
      hardReload(window.location.pathname);
      return;
    }

    const nodes = Array.from(newBody.childNodes).map((node) =>
      document.importNode(node, true)
    );

    document.body.replaceChildren(...nodes);
  }

  function mergeHead(newHead) {
    if (!newHead) {
      return;
    }

    const preservedIds = getPreservedIds();

    const currentHead = document.head;
    const existingById = new Map();

    Array.from(currentHead.children).forEach((node) => {
      if (node.id) {
        existingById.set(node.id, node);
      }
    });

    Array.from(newHead.children).forEach((node) => {
      if (node.id && preservedIds.has(node.id)) {
        return;
      }

      const imported = document.importNode(node, true);

      if (node.id && existingById.has(node.id)) {
        const existing = existingById.get(node.id);
        if (existing) {
          existing.replaceWith(imported);
        }
        existingById.delete(node.id);
      } else {
        currentHead.appendChild(imported);
      }
    });
  }

  function applyCssDiff(path) {
    if (!path) {
      hardReload(window.location.pathname);
      return;
    }

    const origin = window.location.origin;
    const normalizedPath = new URL(path, origin).pathname;
    let updated = false;

    document.querySelectorAll('link[rel="stylesheet"]').forEach((link) => {
      const hrefPath = new URL(link.href, origin).pathname;
      if (hrefPath === normalizedPath) {
        const fresh = new URL(link.href, origin);
        fresh.searchParams.set("_v", Date.now().toString());
        link.href = fresh.toString();
        updated = true;
      }
    });

    if (!updated) {
      hardReload(window.location.pathname);
    }
  }

  function cacheBustUrl(path) {
    const url = new URL(path, window.location.origin);
    url.searchParams.set("_v", Date.now().toString());
    return url.toString();
  }

  function hardReload(path) {
    const url = cacheBustUrl(path);
    window.location.replace(url);
  }

  function pathsMatch(messagePath, currentPath) {
    const normalizedMessage = normalizeHtmlPath(messagePath);
    const normalizedCurrent = normalizeHtmlPath(currentPath);
    return normalizedMessage === normalizedCurrent;
  }

  function normalizeHtmlPath(path) {
    const url = new URL(path, window.location.origin);
    let pathname = url.pathname;
    if (pathname.endsWith("index.html") || pathname.endsWith("index.htm")) {
      pathname = pathname.replace(/index\.html?$/i, "");
    }
    if (pathname.length > 1 && pathname.endsWith("/")) {
      pathname = pathname.replace(/\/+$/, "");
    }
    if (!pathname.startsWith("/")) {
      pathname = `/${pathname}`;
    }
    if (pathname === "") {
      pathname = "/";
    }
    return pathname;
  }

  function getPreservedIds() {
    return new Set(["__web_dev_server_config", "__web_dev_server_client"]);
  }

  function reactivateScripts(root) {
    const preservedIds = getPreservedIds();
    const scripts = root.querySelectorAll("script");
    scripts.forEach((oldScript) => {
      if (oldScript.id && preservedIds.has(oldScript.id)) {
        return;
      }
      const newScript = document.createElement("script");
      Array.from(oldScript.attributes).forEach(({ name, value }) => {
        newScript.setAttribute(name, value);
      });
      if (!oldScript.src) {
        newScript.textContent = oldScript.textContent;
      }
      oldScript.replaceWith(newScript);
    });
  }

  connect();
})();
