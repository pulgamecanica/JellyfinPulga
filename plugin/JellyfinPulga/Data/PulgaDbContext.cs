using Microsoft.EntityFrameworkCore;

namespace JellyfinPulga.Data;

public class PulgaDbContext : DbContext
{
    private readonly string _dbPath;

    public PulgaDbContext(string dbPath)
    {
        _dbPath = dbPath;
    }

    public DbSet<ChatMessage> ChatMessages => Set<ChatMessage>();
    public DbSet<PrivateMessage> PrivateMessages => Set<PrivateMessage>();
    public DbSet<ContentReport> ContentReports => Set<ContentReport>();
    public DbSet<BlockedUser> BlockedUsers => Set<BlockedUser>();

    protected override void OnConfiguring(DbContextOptionsBuilder options) =>
        options.UseSqlite($"Data Source={_dbPath}");

    protected override void OnModelCreating(ModelBuilder model)
    {
        model.Entity<ChatMessage>(e =>
        {
            e.HasIndex(m => new { m.RoomId, m.CreatedAt });
        });

        model.Entity<PrivateMessage>(e =>
        {
            e.HasIndex(m => new { m.ToUserId, m.CreatedAt });
            e.HasIndex(m => new { m.FromUserId, m.CreatedAt });
        });

        model.Entity<ContentReport>(e =>
        {
            e.HasIndex(r => r.Status);
            e.HasIndex(r => r.ItemId);
        });

        model.Entity<BlockedUser>(e =>
        {
            e.HasKey(b => new { b.UserId, b.BlockedUserId });
        });
    }
}

public class ChatMessage
{
    public long Id { get; set; }
    public required string RoomId { get; set; }
    public required Guid UserId { get; set; }
    public required string Username { get; set; }
    public required string Content { get; set; }
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
}

public class PrivateMessage
{
    public long Id { get; set; }
    public required Guid FromUserId { get; set; }
    public required string FromUsername { get; set; }
    public required Guid ToUserId { get; set; }
    public required string ToUsername { get; set; }
    public required string Content { get; set; }
    public bool Read { get; set; }
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
}

public class ContentReport
{
    public long Id { get; set; }
    public required Guid ItemId { get; set; }
    public required string ItemName { get; set; }
    public required Guid ReporterId { get; set; }
    public required string ReporterName { get; set; }
    public required string Reason { get; set; }
    public string Details { get; set; } = "";
    public string Status { get; set; } = "open";
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
}

public class BlockedUser
{
    public required Guid UserId { get; set; }
    public required Guid BlockedUserId { get; set; }
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
}
