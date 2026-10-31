(g) => {
  const hidden = new Set([0x9245, 0x9246, 0x1f00, 0x1f01]);
  const hide = (proto) => {
    if (!proto) return;
    const parameter = proto.getParameter;
    const extension = proto.getExtension;
    const supported = proto.getSupportedExtensions;
    proto.getParameter = function (name) {
      if (!hidden.has(name)) return parameter.call(this, name);
      return name === 0x9245 || name === 0x1f00 ? "WebKit" : "WebKit WebGL";
    };
    proto.getExtension = function (name) {
      return String(name).toLowerCase() === "webgl_debug_renderer_info" ? null : extension.call(this, name);
    };
    proto.getSupportedExtensions = function () {
      return (supported.call(this) || []).filter((name) => name !== "WEBGL_debug_renderer_info");
    };
  };
  hide(g.WebGLRenderingContext && g.WebGLRenderingContext.prototype);
  hide(g.WebGL2RenderingContext && g.WebGL2RenderingContext.prototype);
  const nav = g.Navigator || g.WorkerNavigator;
  if (nav && "gpu" in nav.prototype) Object.defineProperty(nav.prototype, "gpu", { get: () => undefined, configurable: true });
}
