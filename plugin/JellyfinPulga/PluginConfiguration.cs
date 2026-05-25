using MediaBrowser.Model.Plugins;

namespace JellyfinPulga;

public class PluginConfiguration : BasePluginConfiguration
{
    public bool EnableChat { get; set; } = true;
    public bool EnableReporting { get; set; } = true;
    public bool EnablePrivateMessages { get; set; } = true;
    public int MaxChatMessages { get; set; } = 200;
}
