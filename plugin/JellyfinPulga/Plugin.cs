using System;
using System.Collections.Generic;
using MediaBrowser.Common.Configuration;
using MediaBrowser.Common.Plugins;
using MediaBrowser.Model.Plugins;
using MediaBrowser.Model.Serialization;

namespace JellyfinPulga;

public class Plugin : BasePlugin<PluginConfiguration>, IHasWebPages
{
    public Plugin(IApplicationPaths appPaths, IXmlSerializer xmlSerializer)
        : base(appPaths, xmlSerializer)
    {
        Instance = this;
    }

    public static Plugin? Instance { get; private set; }

    public override string Name => "JellyfinPulga";

    public override string Description =>
        "Community tools: per-movie chat, private messaging, content reporting, and media health flagging.";

    public override Guid Id => new("e4a1c0d3-7f2b-4a8e-9d6c-3b5f1e0a2c4d");

    public IEnumerable<PluginPageInfo> GetPages()
    {
        return
        [
            new PluginPageInfo
            {
                Name = "JellyfinPulga",
                EmbeddedResourcePath = "JellyfinPulga.Web.pulga.html",
                EnableInMainMenu = true,
                DisplayName = "Pulga Community"
            },
            new PluginPageInfo
            {
                Name = "JellyfinPulgaJS",
                EmbeddedResourcePath = "JellyfinPulga.Web.pulga.js",
            },
            new PluginPageInfo
            {
                Name = "JellyfinPulgaCSS",
                EmbeddedResourcePath = "JellyfinPulga.Web.pulga.css",
            },
            new PluginPageInfo
            {
                Name = "JellyfinPulgaConfig",
                EmbeddedResourcePath = "JellyfinPulga.Web.config.html",
            }
        ];
    }
}
