local prefix, now = ARGV[1], ARGV[2]
for _, id in ipairs(redis.call('ZRANGEBYSCORE', prefix .. 'scheduled', '-inf', '(' .. now)) do
  local queue = redis.call('HGET', prefix .. 'job:' .. id, 'queue')
  redis.call('ZADD', prefix .. 'queue:' .. queue, id, id)
  redis.call('ZREM', prefix .. 'scheduled', id)
end
local oldest
for i = 3, #ARGV do
  local head = redis.call('ZRANGE', prefix .. 'queue:' .. ARGV[i], 0, 0)[1]
  if head and (not oldest or tonumber(head) < tonumber(oldest)) then
    oldest = head
  end
end
if not oldest then
  return false
end
local job = prefix .. 'job:' .. oldest
local queue, name, args, attempt, unique, enqueued = unpack(redis.call('HMGET', job, 'queue', 'name', 'args', 'attempt', 'unique', 'enqueued_at'))
redis.call('ZREM', prefix .. 'queue:' .. queue, oldest)
redis.call('ZADD', prefix .. 'running', now, oldest)
redis.call('HSET', job, 'started_at', now)
redis.call('HDEL', job, 'finished_at')
if unique then
  redis.call('DEL', prefix .. 'unique:' .. unique)
  redis.call('HDEL', job, 'unique')
end
return { tonumber(oldest), queue, name, args, tonumber(attempt), enqueued }
