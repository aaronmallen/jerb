local prefix = ARGV[1]
for i = 2, #ARGV, 2 do
  local id, cutoff = ARGV[i], tonumber(ARGV[i + 1])
  local claimed = redis.call('ZSCORE', prefix .. 'running', id)
  if claimed and tonumber(claimed) <= cutoff then
    redis.call('ZREM', prefix .. 'running', id)
    local queue = redis.call('HGET', prefix .. 'job:' .. id, 'queue')
    redis.call('ZADD', prefix .. 'queue:' .. queue, id, id)
  end
end
