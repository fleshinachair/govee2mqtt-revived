ARG BUILD_FROM
# The add-on copies its binary out of the standalone image. CI pins this to the
# release being built. The default serves Supervisor local builds, which pass
# only BUILD_FROM.
ARG GOVEE_IMAGE=ghcr.io/fleshinachair/govee2mqtt-revived:latest
FROM ${GOVEE_IMAGE} AS govee2mqtt
FROM $BUILD_FROM
COPY run.sh /run.sh
COPY --from=govee2mqtt /app/govee /app/
COPY --from=govee2mqtt /app/assets /app/assets/
COPY --from=govee2mqtt /app/AmazonRootCA1.pem /app/
CMD [ "/run.sh" ]
