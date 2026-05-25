using System.ComponentModel.DataAnnotations;
using JellyfinPulga.Data;
using MediaBrowser.Common.Configuration;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;

namespace JellyfinPulga.Api;

[ApiController]
[Route("Pulga/Chat")]
[Authorize]
public class ChatController : ControllerBase
{
    private readonly string _dbPath;

    public ChatController(IApplicationPaths appPaths)
    {
        _dbPath = Path.Combine(appPaths.PluginConfigurationsPath, "pulga.db");
    }

    private PulgaDbContext Db() => new(_dbPath);

    [HttpGet("{roomId}/Messages")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> GetMessages(string roomId, [FromQuery] int limit = 50)
    {
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var messages = await db.ChatMessages
            .Where(m => m.RoomId == roomId)
            .OrderByDescending(m => m.CreatedAt)
            .Take(limit)
            .OrderBy(m => m.CreatedAt)
            .ToListAsync();

        return Ok(messages);
    }

    [HttpPost("{roomId}/Send")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> SendMessage(string roomId, [FromBody] SendMessageDto dto)
    {
        var userId = AuthHelper.GetUserId(User);

        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var msg = new ChatMessage
        {
            RoomId = roomId,
            UserId = userId,
            Username = dto.Username,
            Content = dto.Content
        };

        db.ChatMessages.Add(msg);
        await db.SaveChangesAsync();

        return Ok(msg);
    }
}

public record SendMessageDto(
    [Required] string Username,
    [Required] string Content);
