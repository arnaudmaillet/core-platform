CREATE TABLE IF NOT EXISTS content_comments.comments_by_post (
    post_id uuid,
    comment_id uuid,
    profile_id uuid,
    content text,
    edited_at timestamp,
    updated_at timestamp,
    PRIMARY KEY ((post_id), comment_id)
) WITH CLUSTERING ORDER BY (comment_id DESC);

CREATE TABLE IF NOT EXISTS content_comments.replies_by_comment (
    parent_comment_id uuid,
    comment_id uuid,
    post_id uuid,
    profile_id uuid,
    content text,
    edited_at timestamp,
    updated_at timestamp,
    PRIMARY KEY ((parent_comment_id), comment_id)
) WITH CLUSTERING ORDER BY (comment_id ASC)

CREATE TABLE IF NOT EXISTS content_comments.comment_user_profiles (
    profile_id uuid,
    username text,
    display_name text,
    avatar_url text,
    updated_at timestamp,
    PRIMARY KEY (profile_id)
);