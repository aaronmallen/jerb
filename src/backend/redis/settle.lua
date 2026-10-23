local prefix, id, outcome, message, trace, at, now = unpack(ARGV)
if redis.call('ZREM', prefix .. 'running', id) == 0 then
  return 0
end
local job = prefix .. 'job:' .. id
if outcome == 'finished' then
  redis.call('DEL', job)
elseif outcome == 'failed' then
  redis.call('HSET', job, 'error', message, 'backtrace', trace, 'finished_at', now)
  redis.call('ZADD', prefix .. 'dead', id, id)
else
  redis.call('HSET', job, 'error', message, 'backtrace', trace, 'finished_at', now)
  redis.call('HINCRBY', job, 'attempt', 1)
  redis.call('ZADD', prefix .. 'scheduled', at, id)
end
return 1
