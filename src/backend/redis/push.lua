local prefix, queue, name, args, at, unique, schedule, fired_at, now = unpack(ARGV)
if schedule ~= '' then
  local last = redis.call('HGET', prefix .. 'schedules', schedule)
  if last and tonumber(last) >= tonumber(fired_at) then
    return false
  end
  redis.call('HSET', prefix .. 'schedules', schedule, fired_at)
end
if unique ~= '' then
  local held = redis.call('GET', prefix .. 'unique:' .. unique)
  if held then
    return tonumber(held)
  end
end
local id = redis.call('INCR', prefix .. 'id')
local job = prefix .. 'job:' .. id
redis.call('HSET', job, 'queue', queue, 'name', name, 'args', args, 'attempt', 0, 'enqueued_at', now)
if unique ~= '' then
  redis.call('SET', prefix .. 'unique:' .. unique, id)
  redis.call('HSET', job, 'unique', unique)
end
if at == '' then
  redis.call('ZADD', prefix .. 'queue:' .. queue, id, id)
else
  redis.call('ZADD', prefix .. 'scheduled', at, id)
end
return id
