//! Provider-agnostic git-forge events and their mapping to
//! `ChannelMessage`s.

use chrono::{DateTime, Utc};
use serde_json::json;
use zeroclaw_api::channel::{CHANNEL_SOP_SUBJECT_PREFIX, ChannelMessage};

use super::types::{
    EVT_ISSUE_COMMENT_CREATED, EVT_ISSUES_OPENED, EVT_PR_REVIEW_COMMENT_CREATED,
    EVT_PULL_REQUEST_CLOSED, EVT_PULL_REQUEST_MERGED, EVT_PULL_REQUEST_OPENED,
    EVT_PULL_REQUEST_REVIEW_REQUESTED, EVT_RELEASE_PUBLISHED, EVT_WORKFLOW_RUN_COMPLETED,
    EVT_WORKFLOW_RUN_FAILED, IssueRef, RepoRef,
};

#[derive(Debug, Clone)]
pub struct EventActor {
    pub login: String,
    pub is_bot: bool,
}

pub struct EventFilter<'a> {
    pub bot_login: &'a str,
    pub mention_handle: &'a str,
    pub mention_only: bool,
    pub listen_to_bots: bool,
}

impl EventFilter<'_> {
    fn admit_author(&self, author: &EventActor) -> bool {
        if author.login.eq_ignore_ascii_case(self.bot_login) {
            return false;
        }
        !author.is_bot || self.listen_to_bots
    }

    fn admit_text(&self, body: &str, gated: bool) -> Option<String> {
        if gated && self.mention_only && !contains_mention(body, self.mention_handle) {
            return None;
        }
        let content = strip_mention(body, self.mention_handle);
        if content.is_empty() {
            return None;
        }
        Some(content)
    }
}

#[derive(Debug, Clone)]
pub struct CommentPost {
    pub repo: RepoRef,
    pub number: u64,
    pub comment_id: String,
    pub author: EventActor,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IssuePost {
    pub repo: RepoRef,
    pub number: u64,
    pub issue_id: String,
    pub author: EventActor,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PullPost {
    pub repo: RepoRef,
    pub number: u64,
    pub author: EventActor,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PullTransition {
    pub repo: RepoRef,
    pub number: u64,
    pub title: String,
    pub author: EventActor,
    pub html_url: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub repo: RepoRef,
    pub run_id: String,
    pub attempt: u64,
    pub name: String,
    pub branch: Option<String>,
    pub run_number: u64,
    pub pr_number: Option<u64>,
    pub actor: Option<EventActor>,
    pub html_url: String,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReleasePost {
    pub repo: RepoRef,
    pub release_id: String,
    pub tag: String,
    pub name: Option<String>,
    pub author: EventActor,
    pub body: String,
    pub html_url: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum GitEvent {
    IssueCommentCreated(CommentPost),
    IssueOpened(IssuePost),
    PullRequestOpened(PullPost),
    PullRequestClosed(PullTransition),
    PullRequestMerged(PullTransition),
    PullRequestReviewRequested(PullTransition),
    PullRequestReviewCommentCreated(CommentPost),
    WorkflowRunCompleted(RunOutcome),
    WorkflowRunFailed(RunOutcome),
    ReleasePublished(ReleasePost),
}

impl GitEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::IssueCommentCreated(_) => EVT_ISSUE_COMMENT_CREATED,
            Self::IssueOpened(_) => EVT_ISSUES_OPENED,
            Self::PullRequestOpened(_) => EVT_PULL_REQUEST_OPENED,
            Self::PullRequestClosed(_) => EVT_PULL_REQUEST_CLOSED,
            Self::PullRequestMerged(_) => EVT_PULL_REQUEST_MERGED,
            Self::PullRequestReviewRequested(_) => EVT_PULL_REQUEST_REVIEW_REQUESTED,
            Self::PullRequestReviewCommentCreated(_) => EVT_PR_REVIEW_COMMENT_CREATED,
            Self::WorkflowRunCompleted(_) => EVT_WORKFLOW_RUN_COMPLETED,
            Self::WorkflowRunFailed(_) => EVT_WORKFLOW_RUN_FAILED,
            Self::ReleasePublished(_) => EVT_RELEASE_PUBLISHED,
        }
    }

    pub fn dedup_id(&self) -> String {
        match self {
            Self::IssueCommentCreated(p) => format!("ghc_{}", p.comment_id),
            Self::IssueOpened(p) => format!("ghi_{}", p.issue_id),
            Self::PullRequestOpened(p) => format!("ghpr_{}#{}", p.repo, p.number),
            Self::PullRequestClosed(t)
            | Self::PullRequestMerged(t)
            | Self::PullRequestReviewRequested(t) => {
                format!("ghprx_{}#{}@{}", t.repo, t.number, t.at.timestamp())
            }
            Self::PullRequestReviewCommentCreated(p) => format!("ghrc_{}", p.comment_id),
            Self::WorkflowRunCompleted(r) | Self::WorkflowRunFailed(r) => {
                format!("ghwr_{}_{}", r.run_id, r.attempt)
            }
            Self::ReleasePublished(r) => format!("ghrel_{}", r.release_id),
        }
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            Self::IssueCommentCreated(p) | Self::PullRequestReviewCommentCreated(p) => p.created_at,
            Self::IssueOpened(p) => p.created_at,
            Self::PullRequestOpened(p) => p.created_at,
            Self::PullRequestClosed(t)
            | Self::PullRequestMerged(t)
            | Self::PullRequestReviewRequested(t) => t.at,
            Self::WorkflowRunCompleted(r) | Self::WorkflowRunFailed(r) => r.finished_at,
            Self::ReleasePublished(r) => r.published_at,
        }
    }

    pub fn author(&self) -> Option<&EventActor> {
        match self {
            Self::IssueCommentCreated(p) | Self::PullRequestReviewCommentCreated(p) => Some(&p.author),
            Self::IssueOpened(p) => Some(&p.author),
            Self::PullRequestOpened(p) => Some(&p.author),
            Self::PullRequestClosed(t)
            | Self::PullRequestMerged(t)
            | Self::PullRequestReviewRequested(t) => Some(&t.author),
            Self::WorkflowRunCompleted(r) | Self::WorkflowRunFailed(r) => r.actor.as_ref(),
            Self::ReleasePublished(r) => Some(&r.author),
        }
    }

    pub fn is_conversational(&self) -> bool {
        matches!(
            self,
            Self::IssueCommentCreated(_)
                | Self::IssueOpened(_)
                | Self::PullRequestOpened(_)
                | Self::PullRequestReviewCommentCreated(_)
                | Self::PullRequestReviewRequested(_)
        )
    }
}

pub fn event_to_message(
    event: &GitEvent,
    filter: &EventFilter<'_>,
    channel_key: &str,
    alias: &str,
    gate_mentions: bool,
) -> Option<ChannelMessage> {
    let author = event.author()?;
    if !filter.admit_author(author) {
        return None;
    }

    let gated = gate_mentions && event.is_conversational();
    let id = event.dedup_id();
    let timestamp = event.created_at().timestamp();
    let sender = author.login.clone();
    let ctx = MessageCtx { channel_key, alias };

    match event {
        GitEvent::IssueCommentCreated(p) | GitEvent::PullRequestReviewCommentCreated(p) => {
            let content = filter.admit_text(&p.body, gated)?;
            let target = issue_target(&p.repo, p.number);
            Some(message(id, sender, &target, content, timestamp, None, &ctx))
        }
        GitEvent::PullRequestReviewRequested(t) => {
            let mut content = format!(
                "Review requested for pull request {}#{} ({})",
                t.repo, t.number, t.title
            );
            if !t.html_url.is_empty() {
                content.push_str(&format!("\n{}", t.html_url));
            }
            let target = issue_target(&t.repo, t.number);
            let subject = Some(t.title.clone());
            Some(message(id, sender, &target, content, timestamp, subject, &ctx))
        }
        GitEvent::IssueOpened(p) => {
            let content = opening_content(filter, &p.body, gated, || {
                format!("Issue #{} opened: {}", p.number, p.title)
            })?;
            let target = issue_target(&p.repo, p.number);
            let subject = Some(p.title.clone());
            Some(message(
                id, sender, &target, content, timestamp, subject, &ctx,
            ))
        }
        GitEvent::PullRequestOpened(p) => {
            let content = opening_content(filter, &p.body, gated, || {
                format!("Pull request #{} opened: {}", p.number, p.title)
            })?;
            let target = issue_target(&p.repo, p.number);
            let subject = Some(p.title.clone());
            Some(message(
                id, sender, &target, content, timestamp, subject, &ctx,
            ))
        }
        GitEvent::PullRequestClosed(t) | GitEvent::PullRequestMerged(t) => {
            let verb = if matches!(event, GitEvent::PullRequestMerged(_)) {
                "merged"
            } else {
                "closed without merging"
            };
            let mut content = format!(
                "Pull request {}#{} ({}) was {verb}.",
                t.repo, t.number, t.title
            );
            if !t.html_url.is_empty() {
                content.push_str(&format!("\n{}", t.html_url));
            }
            let target = issue_target(&t.repo, t.number);
            let subject = Some(t.title.clone());
            Some(message(
                id, sender, &target, content, timestamp, subject, &ctx,
            ))
        }
        GitEvent::WorkflowRunCompleted(r) | GitEvent::WorkflowRunFailed(r) => {
            let verdict = if matches!(event, GitEvent::WorkflowRunFailed(_)) {
                "failed"
            } else {
                "completed successfully"
            };
            let mut content = format!("Workflow run {verdict}: {} #{}", r.name, r.run_number);
            if let Some(branch) = &r.branch {
                content.push_str(&format!(" on {branch}"));
            }
            if r.attempt > 1 {
                content.push_str(&format!(" (attempt {})", r.attempt));
            }
            if !r.html_url.is_empty() {
                content.push_str(&format!("\n{}", r.html_url));
            }
            let target = match r.pr_number {
                Some(n) => issue_target(&r.repo, n),
                None => r.repo.to_string(),
            };
            let subject = Some(r.name.clone());
            Some(message(
                id, sender, &target, content, timestamp, subject, &ctx,
            ))
        }
        GitEvent::ReleasePublished(r) => {
            let mut content = format!("Release {} published", r.tag);
            if !r.html_url.is_empty() {
                content.push_str(&format!(": {}", r.html_url));
            }
            if !r.body.trim().is_empty() {
                content.push_str(&format!("\n\n{}", r.body.trim()));
            }
            let subject = Some(format!(
                "Release {}",
                r.name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| r.tag.clone())
            ));
            Some(message(
                id,
                sender,
                &r.repo.to_string(),
                content,
                timestamp,
                subject,
                &ctx,
            ))
        }
    }
}

pub fn event_to_sop_message(
    event: &GitEvent,
    filter: &EventFilter<'_>,
    channel_key: &str,
    alias: &str,
    provider: &str,
    sop: &str,
) -> Option<ChannelMessage> {
    let author = event.author()?;
    if !filter.admit_author(author) {
        return None;
    }

    let id = event.dedup_id();
    let timestamp = event.created_at().timestamp();
    let target = event_target(event);
    let topic = sop_topic(channel_key, alias, event.event_type());
    let payload = event_payload(event, channel_key, alias, provider, sop, &topic);
    let ctx = MessageCtx { channel_key, alias };
    let mut msg = message(
        id,
        author.login.clone(),
        &target,
        payload.to_string(),
        timestamp,
        Some(format!("{CHANNEL_SOP_SUBJECT_PREFIX}{topic}")),
        &ctx,
    );
    msg.internal_sop_event = Some(topic);
    Some(msg)
}

fn sop_topic(channel_key: &str, alias: &str, event_type: &str) -> String {
    zeroclaw_api::channel::ChannelSopTopic::build(channel_key, alias, event_type)
}

fn event_target(event: &GitEvent) -> String {
    match event {
        GitEvent::IssueCommentCreated(p) | GitEvent::PullRequestReviewCommentCreated(p) => {
            issue_target(&p.repo, p.number)
        }
        GitEvent::IssueOpened(p) => issue_target(&p.repo, p.number),
        GitEvent::PullRequestOpened(p) => issue_target(&p.repo, p.number),
        GitEvent::PullRequestClosed(t)
        | GitEvent::PullRequestMerged(t)
        | GitEvent::PullRequestReviewRequested(t) => issue_target(&t.repo, t.number),
        GitEvent::WorkflowRunCompleted(r) | GitEvent::WorkflowRunFailed(r) => match r.pr_number {
            Some(number) => issue_target(&r.repo, number),
            None => r.repo.to_string(),
        },
        GitEvent::ReleasePublished(r) => r.repo.to_string(),
    }
}

fn actor_payload(actor: &EventActor) -> serde_json::Value {
    json!({
        "login": actor.login,
        "is_bot": actor.is_bot,
    })
}

fn common_payload(
    event: &GitEvent,
    channel_key: &str,
    alias: &str,
    provider: &str,
    sop: &str,
    topic: &str,
) -> serde_json::Value {
    json!({
        "source": "channel",
        "channel": channel_key,
        "channel_alias": alias,
        "provider": provider,
        "sop": sop,
        "topic": topic,
        "event_type": event.event_type(),
        "dedup_id": event.dedup_id(),
        "created_at": event.created_at().to_rfc3339(),
        "target": event_target(event),
    })
}

fn event_payload(
    event: &GitEvent,
    channel_key: &str,
    alias: &str,
    provider: &str,
    sop: &str,
    topic: &str,
) -> serde_json::Value {
    let mut payload = common_payload(event, channel_key, alias, provider, sop, topic);
    let obj = payload
        .as_object_mut()
        .expect("common_payload produces a JSON object");
    match event {
        GitEvent::IssueCommentCreated(p) | GitEvent::PullRequestReviewCommentCreated(p) => {
            obj.insert("repo".into(), json!(p.repo.to_string()));
            obj.insert("number".into(), json!(p.number));
            obj.insert("comment_id".into(), json!(p.comment_id));
            obj.insert("author".into(), actor_payload(&p.author));
            obj.insert("body".into(), json!(p.body));
        }
        GitEvent::IssueOpened(p) => {
            obj.insert("repo".into(), json!(p.repo.to_string()));
            obj.insert("number".into(), json!(p.number));
            obj.insert("issue_id".into(), json!(p.issue_id));
            obj.insert("author".into(), actor_payload(&p.author));
            obj.insert("title".into(), json!(p.title));
            obj.insert("body".into(), json!(p.body));
        }
        GitEvent::PullRequestOpened(p) => {
            obj.insert("repo".into(), json!(p.repo.to_string()));
            obj.insert("number".into(), json!(p.number));
            obj.insert("author".into(), actor_payload(&p.author));
            obj.insert("title".into(), json!(p.title));
            obj.insert("body".into(), json!(p.body));
        }
        GitEvent::PullRequestClosed(t) | GitEvent::PullRequestMerged(t) | GitEvent::PullRequestReviewRequested(t) => {
            obj.insert("repo".into(), json!(t.repo.to_string()));
            obj.insert("number".into(), json!(t.number));
            obj.insert("author".into(), actor_payload(&t.author));
            obj.insert("title".into(), json!(t.title));
            obj.insert("html_url".into(), json!(t.html_url));
        }
        GitEvent::WorkflowRunCompleted(r) | GitEvent::WorkflowRunFailed(r) => {
            obj.insert("repo".into(), json!(r.repo.to_string()));
            obj.insert("run_id".into(), json!(r.run_id));
            obj.insert("attempt".into(), json!(r.attempt));
            obj.insert("name".into(), json!(r.name));
            obj.insert("branch".into(), json!(r.branch));
            obj.insert("run_number".into(), json!(r.run_number));
            obj.insert("pr_number".into(), json!(r.pr_number));
            obj.insert("actor".into(), r.actor.as_ref().map(actor_payload).into());
            obj.insert("html_url".into(), json!(r.html_url));
        }
        GitEvent::ReleasePublished(r) => {
            obj.insert("repo".into(), json!(r.repo.to_string()));
            obj.insert("release_id".into(), json!(r.release_id));
            obj.insert("tag".into(), json!(r.tag));
            obj.insert("name".into(), json!(r.name));
            obj.insert("author".into(), actor_payload(&r.author));
            obj.insert("body".into(), json!(r.body));
            obj.insert("html_url".into(), json!(r.html_url));
        }
    }
    payload
}

struct MessageCtx<'a> {
    channel_key: &'a str,
    alias: &'a str,
}

fn opening_content(
    filter: &EventFilter<'_>,
    body: &str,
    gated: bool,
    fallback: impl FnOnce() -> String,
) -> Option<String> {
    match filter.admit_text(body, gated) {
        Some(content) => Some(content),
        None if !gated => Some(fallback()),
        None => None,
    }
}

fn issue_target(repo: &RepoRef, number: u64) -> String {
    IssueRef {
        repo: repo.clone(),
        number,
    }
    .to_string()
}

fn message(
    id: String,
    sender: String,
    target: &str,
    content: String,
    timestamp: i64,
    subject: Option<String>,
    ctx: &MessageCtx<'_>,
) -> ChannelMessage {
    ChannelMessage {
        id,
        sender,
        reply_target: target.to_string(),
        content,
        channel: ctx.channel_key.to_string(),
        channel_alias: Some(ctx.alias.to_string()),
        timestamp: timestamp.max(0) as u64,
        thread_ts: Some(target.to_string()),
        subject,
        ..ChannelMessage::default()
    }
}

pub fn contains_mention(body: &str, handle: &str) -> bool {
    if handle.is_empty() {
        return false;
    }
    let body_lower = body.to_ascii_lowercase();
    let needle = format!("@{}", handle.to_ascii_lowercase());
    let mut start = 0;
    while let Some(pos) = body_lower[start..].find(&needle) {
        let end = start + pos + needle.len();
        let boundary = body_lower[end..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
        if boundary {
            return true;
        }
        start = end;
    }
    false
}

pub fn strip_mention(body: &str, handle: &str) -> String {
    if handle.is_empty() {
        return body.trim().to_string();
    }
    let needle = format!("@{}", handle.to_ascii_lowercase());
    let mut out = String::with_capacity(body.len());
    let body_lower = body.to_ascii_lowercase();
    let mut idx = 0;
    while let Some(pos) = body_lower[idx..].find(&needle) {
        let abs = idx + pos;
        let end = abs + needle.len();
        let boundary = body_lower[end..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
        out.push_str(&body[idx..abs]);
        if !boundary {
            out.push_str(&body[abs..end]);
        }
        idx = end;
    }
    out.push_str(&body[idx..]);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(mention_only: bool, listen_to_bots: bool) -> EventFilter<'static> {
        EventFilter {
            bot_login: "myapp[bot]",
            mention_handle: "myapp",
            mention_only,
            listen_to_bots,
        }
    }

    fn repo() -> RepoRef {
        RepoRef::parse("octo/repo").unwrap()
    }

    fn actor(login: &str, is_bot: bool) -> EventActor {
        EventActor {
            login: login.to_string(),
            is_bot,
        }
    }

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn to_message(
        event: &GitEvent,
        filter: &EventFilter<'_>,
        gate: bool,
    ) -> Option<ChannelMessage> {
        event_to_message(event, filter, "git", "main", gate)
    }

    fn comment_event(login: &str, is_bot: bool, body: &str, number: u64) -> GitEvent {
        GitEvent::IssueCommentCreated(CommentPost {
            repo: repo(),
            number,
            comment_id: "9001".to_string(),
            author: actor(login, is_bot),
            body: body.to_string(),
            created_at: at("2026-06-13T01:02:03Z"),
        })
    }

    #[test]
    fn issue_comment_maps_with_issue_threading() {
        let event = comment_event("test_user", false, "@myapp run the tests", 7);
        assert_eq!(event.event_type(), "issue_comment.created");
        assert_eq!(event.dedup_id(), "ghc_9001");
        assert!(event.is_conversational());

        let msg = to_message(&event, &filter(true, false), true).unwrap();
        assert_eq!(msg.id, "ghc_9001");
        assert_eq!(msg.sender, "test_user");
        assert_eq!(msg.reply_target, "octo/repo#7");
        assert_eq!(msg.thread_ts.as_deref(), Some("octo/repo#7"));
        assert_eq!(msg.content, "run the tests");
        assert_eq!(msg.channel, "git");
        assert_eq!(msg.channel_alias.as_deref(), Some("main"));
    }

    #[test]
    fn opening_posts_carry_subject_and_pr_identity() {
        let issue = GitEvent::IssueOpened(IssuePost {
            repo: repo(),
            number: 12,
            issue_id: "555".to_string(),
            author: actor("test_user", false),
            title: "Flaky test".to_string(),
            body: "@myapp please investigate".to_string(),
            created_at: at("2026-06-13T01:00:00Z"),
        });
        assert_eq!(issue.event_type(), "issues.opened");
        assert_eq!(issue.dedup_id(), "ghi_555");
        let msg = to_message(&issue, &filter(true, false), true).unwrap();
        assert_eq!(msg.id, "ghi_555");
        assert_eq!(msg.reply_target, "octo/repo#12");
        assert_eq!(msg.subject.as_deref(), Some("Flaky test"));
        assert_eq!(msg.content, "please investigate");

        let pr = GitEvent::PullRequestOpened(PullPost {
            repo: repo(),
            number: 12,
            author: actor("test_user", false),
            title: "Flaky test".to_string(),
            body: "x".to_string(),
            created_at: at("2026-06-13T01:00:00Z"),
        });
        assert_eq!(pr.event_type(), "pull_request.opened");
        assert_eq!(pr.dedup_id(), "ghpr_octo/repo#12");
    }

    #[test]
    fn review_request_maps_to_message() {
        let event = GitEvent::PullRequestReviewRequested(PullTransition {
            repo: repo(),
            number: 12,
            title: "Flaky test".to_string(),
            author: actor("test_user", false),
            html_url: "https://github.com/octo/repo/pull/12".to_string(),
            at: at("2026-06-13T01:03:00Z"),
        });
        assert_eq!(event.event_type(), "pull_request.review_requested");
        let msg = to_message(&event, &filter(true, false), true).unwrap();
        assert_eq!(msg.reply_target, "octo/repo#12");
        assert!(msg.content.contains("Review requested for pull request"));
    }
}

