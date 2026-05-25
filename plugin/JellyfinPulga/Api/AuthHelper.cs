using System.Security.Claims;

namespace JellyfinPulga.Api;

public static class AuthHelper
{
    private const string JellyfinUserIdClaim = "Jellyfin-UserId";

    public static Guid GetUserId(ClaimsPrincipal principal)
    {
        var value = principal.FindFirst(JellyfinUserIdClaim)?.Value;
        return string.IsNullOrEmpty(value) ? Guid.Empty : Guid.Parse(value);
    }
}
