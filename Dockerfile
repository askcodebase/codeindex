FROM redis:alpine

# Expose port 6739 
EXPOSE 6739

# Set default Redis config
RUN echo "port 6739" >> /usr/local/etc/redis/redis.conf

# Start Redis
CMD [ "redis-server", "/usr/local/etc/redis/redis.conf" ]
