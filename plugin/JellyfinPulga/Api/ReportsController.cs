using System.ComponentModel.DataAnnotations;
using JellyfinPulga.Data;
using MediaBrowser.Common.Api;
using MediaBrowser.Common.Configuration;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;

namespace JellyfinPulga.Api;

[ApiController]
[Route("Pulga/Reports")]
[Authorize]
public class ReportsController : ControllerBase
{
    private readonly string _dbPath;

    public ReportsController(IApplicationPaths appPaths)
    {
        _dbPath = Path.Combine(appPaths.PluginConfigurationsPath, "pulga.db");
    }

    private PulgaDbContext Db() => new(_dbPath);

    [HttpGet]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> GetReports([FromQuery] string? status = null)
    {
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var query = db.ContentReports.AsQueryable();
        if (!string.IsNullOrEmpty(status))
        {
            query = query.Where(r => r.Status == status);
        }

        var reports = await query
            .OrderByDescending(r => r.CreatedAt)
            .ToListAsync();

        return Ok(reports);
    }

    [HttpPost]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> CreateReport([FromBody] CreateReportDto dto)
    {
        var userId = AuthHelper.GetUserId(User);
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var report = new ContentReport
        {
            ItemId = dto.ItemId,
            ItemName = dto.ItemName,
            ReporterId = userId,
            ReporterName = dto.ReporterName,
            Reason = dto.Reason,
            Details = dto.Details ?? ""
        };

        db.ContentReports.Add(report);
        await db.SaveChangesAsync();

        return Ok(report);
    }

    [HttpPost("{id}/Status")]
    [Authorize(Policy = Policies.RequiresElevation)]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> UpdateStatus(long id, [FromBody] UpdateStatusDto dto)
    {
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var updated = await db.ContentReports
            .Where(r => r.Id == id)
            .ExecuteUpdateAsync(s => s.SetProperty(r => r.Status, dto.Status));

        return updated > 0
            ? Ok(new { ok = true })
            : NotFound(new { error = "report not found" });
    }

    [HttpGet("Export")]
    [ProducesResponseType(StatusCodes.Status200OK)]
    public async Task<ActionResult> Export()
    {
        await using var db = Db();
        await db.Database.EnsureCreatedAsync();

        var reports = await db.ContentReports
            .Where(r => r.Status == "open" || r.Status == "reviewed")
            .OrderBy(r => r.ItemName)
            .ToListAsync();

        var lines = new List<string> { "Name\tReason\tStatus\tReporter\tDetails" };
        foreach (var r in reports)
        {
            lines.Add($"{r.ItemName}\t{r.Reason}\t{r.Status}\t{r.ReporterName}\t{r.Details}");
        }

        return Content(string.Join("\n", lines), "text/tab-separated-values");
    }
}

public record CreateReportDto(
    [Required] Guid ItemId,
    [Required] string ItemName,
    [Required] string ReporterName,
    [Required] string Reason,
    string? Details);

public record UpdateStatusDto([Required] string Status);
