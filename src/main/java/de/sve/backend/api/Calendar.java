package de.sve.backend.api;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse.BodyHandlers;
import java.util.List;

import javax.ws.rs.GET;
import javax.ws.rs.HeaderParam;
import javax.ws.rs.POST;
import javax.ws.rs.Path;
import javax.ws.rs.Produces;
import javax.ws.rs.core.MediaType;
import javax.ws.rs.core.Response;
import javax.ws.rs.core.Response.Status;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.CalendarManager;
import de.sve.backend.model.calendar.Appointment;

@Path("/calendar")
@SuppressWarnings("static-method")
public class Calendar {

	private static final Logger LOG = LoggerFactory.getLogger(Calendar.class);

	private static final String RE_DEPLOY_HOOK = "https://api.netlify.com/build_hooks/5ede8485bae5450298c17bc4"; //$NON-NLS-1$

	@Path("/appointments")
	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<Appointment> appointments() throws BackendException {
		try {
			return CalendarManager.appointments();
		} catch (Throwable t) {
			String message = "Error while loading apointments"; //$NON-NLS-1$
			LOG.error(message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/notifications")
	@POST
	public Response notifications(@HeaderParam("X-Goog-Channel-Id") String channelID) {
		LOG.info("Recieved calendar notification for channel id " + channelID); //$NON-NLS-1$
		try {
			int statusCode = triggerReDeploy();
			if (statusCode == 200) {
				LOG.info("Re-Deploy triggered successfully"); //$NON-NLS-1$
			} else {
				LOG.warn("Trigger Re-Deploy failed with status code " + statusCode); //$NON-NLS-1$
			}
		} catch (IOException | InterruptedException e) {
			LOG.error("Trigger Re-Deploy failed", e); //$NON-NLS-1$
		}
		return Response.status(Status.OK).build();
	}

	private static int triggerReDeploy() throws IOException, InterruptedException {
		HttpClient client = HttpClient.newHttpClient();
		HttpRequest request = HttpRequest.newBuilder()
										 .uri(URI.create(RE_DEPLOY_HOOK))
										 .POST(HttpRequest.BodyPublishers.ofString("")) //$NON-NLS-1$
										 .build();
		return client.send(request, BodyHandlers.ofString()).statusCode();
	}

}
