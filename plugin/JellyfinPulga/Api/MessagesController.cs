using System.ComponentModel.DataAnnotations;
using JellyfinPulga.Data;
using MediaBrowser.Common.Configuration;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;

namespace JellyfinPulga.Api;

[ApiController]
[Route("Pulga/Messages")]
[Authorize]
public class MessagesController : ControllerBase
{
    private readonly string _dbPath;

    public MessagesController(IApplicationPaths appPaths)
    {
        _dbPath = Path.Combine(appPaths.PluginConfigurationsPath, "pulga.db");
    }

    private PulgaDbContext Db() => new(_dbPath);

    [HttpGet("Conversations")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> GetConversations()
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var conversations = await db.PrivateMessages
            .Where(m => m.FromUserId == userId || m.ToUserId == userId)
            .GroupBy(m => m.FromUserId == userId ? m.ToUserId : m.FromUserId)
            .Select(g => new
            {
                UserId = g.Key,
                Username = g.OrderByDescending(m => m.CreatedAt).First().FromUserId == userId
                    ? g.OrderByDescending(m => m.CreatedAt).First().ToUsername
                    : g.OrderByDescending(m => m.CreatedAt).First().FromUsername,
                Unread = g.Count(m => m.ToUserId == userId && !m.Read),
                LastMessage = g.Max(m => m.CreatedAt)
            })
            .OrderByDescending(c => c.LastMessage)
            .ToListAsync();

        return Ok(conversations);
    }

    [HttpGet("{otherUserId}")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> GetMessages(Guid otherUserId, [FromQuery] int limit = 50)
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var blocked = await db.BlockedUsers.AnyAsync(b =>
            (b.UserId == userId && b.BlockedUserId == otherUserId) ||
            (b.UserId == otherUserId && b.BlockedUserId == userId));

        if (blocked)
        {
            return StatusCode(StatusCodes.Status403Forbidden, new { error = "blocked" });
        }

        var messages = await db.PrivateMessages
            .Where(m => (m.FromUserId == userId && m.ToUserId == otherUserId)
                     || (m.FromUserId == otherUserId && m.ToUserId == userId))
            .OrderByDescending(m => m.CreatedAt)
            .Take(limit)
            .OrderBy(m => m.CreatedAt)
            .ToListAsync();

        return Ok(messages);
    }

    [HttpPost("{otherUserId}/Send")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> SendMessage(Guid otherUserId, [FromBody] SendPmDto dto)
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var blocked = await db.BlockedUsers.AnyAsync(b =>
            b.UserId == otherUserId && b.BlockedUserId == userId);

        if (blocked)
        {
            return StatusCode(StatusCodes.Status403Forbidden, new { error = "you are blocked by this user" });
        }

        var msg = new PrivateMessage
        {
            FromUserId = userId,
            FromUsername = dto.FromUsername,
            ToUserId = otherUserId,
            ToUsername = dto.ToUsername,
            Content = dto.Content
        };

        db.PrivateMessages.Add(msg);
        await db.SaveChangesAsync();

        return Ok(msg);
    }

    [HttpPost("{otherUserId}/Read")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> MarkRead(Guid otherUserId)
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        await db.PrivateMessages
            .Where(m => m.ToUserId == userId && m.FromUserId == otherUserId && !m.Read)
            .ExecuteUpdateAsync(s => s.SetProperty(m => m.Read, true));

        return Ok(new { ok = true });
    }

    [HttpPost("Block/{blockedUserId}")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> BlockUser(Guid blockedUserId)
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        if (!await db.BlockedUsers.AnyAsync(b => b.UserId == userId && b.BlockedUserId == blockedUserId))
        {
            db.BlockedUsers.Add(new BlockedUser { UserId = userId, BlockedUserId = blockedUserId });
            await db.SaveChangesAsync();
        }

        return Ok(new { ok = true });
    }

    [HttpPost("Unblock/{blockedUserId}")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> UnblockUser(Guid blockedUserId)
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        await db.BlockedUsers
            .Where(b => b.UserId == userId && b.BlockedUserId == blockedUserId)
            .ExecuteDeleteAsync();

        return Ok(new { ok = true });
    }
}

public record SendPmDto(
    [Required] string FromUsername,
    [Required] string ToUsername,
    [Required] string Content);
