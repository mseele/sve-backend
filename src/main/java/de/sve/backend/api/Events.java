package de.sve.backend.api;

import java.util.List;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.Utils;
import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.EventsManager;
import de.sve.backend.model.events.BookingResponse;
import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventBooking;
import de.sve.backend.model.events.EventCounter;
import jakarta.ws.rs.Consumes;
import jakarta.ws.rs.GET;
import jakarta.ws.rs.POST;
import jakarta.ws.rs.Path;
import jakarta.ws.rs.Produces;
import jakarta.ws.rs.QueryParam;
import jakarta.ws.rs.core.MediaType;
import jakarta.ws.rs.core.Response;
import jakarta.ws.rs.core.Response.Status;

@Path("/events")
@SuppressWarnings("static-method")
public class Events {

	private static final Logger LOG = LoggerFactory.getLogger(Events.class);

	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<Event> getEvents(@QueryParam("beta") boolean beta) throws BackendException {
		try {
			return EventsManager.events(beta);
		} catch (Throwable t) {
			String message = "Error while loading events"; //$NON-NLS-1$
			LOG.error(message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/counter")
	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<EventCounter> getEventsCounter() throws BackendException {
		try {
			return EventsManager.eventsCounter();
		} catch (Throwable t) {
			String message = "Error while loading events"; //$NON-NLS-1$
			LOG.error(message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/booking")
	@POST
	@Consumes(MediaType.APPLICATION_JSON)
	@Produces(MediaType.APPLICATION_JSON)
	public BookingResponse booking(EventBooking booking) {
		return EventsManager.booking(booking);
	}

	@Path("/prebooking")
	@POST
	@Consumes(MediaType.TEXT_PLAIN)
	@Produces(MediaType.APPLICATION_JSON)
	public BookingResponse prebooking(String hash) {
		return EventsManager.prebooking(hash);
	}

	@Path("/update")
	@POST
	@Consumes(MediaType.APPLICATION_JSON)
	public Response update(Event update) {
		try {
			Event event = EventsManager.update(update);
			LOG.info( "Event (" + event.id() + ") has been updated"); //$NON-NLS-1$ //$NON-NLS-2$
			return Response.status(Status.OK).entity(Utils.gson().toJson(event)).build();
		} catch (Throwable t) {
			LOG.error("Could not save new event: " + update, t); //$NON-NLS-1$
			return Response.status(Status.BAD_REQUEST).entity(t).build();
		}
	}

	@Path("/delete")
	@POST
	@Consumes(MediaType.APPLICATION_JSON)
	public Response delete(Event event) {
		try {
			EventsManager.delete(event);
			LOG.info("Event (" + event.id() + ") has been deleted"); //$NON-NLS-1$ //$NON-NLS-2$
			return Response.status(Status.OK).build();
		} catch (Throwable t) {
			LOG.error("Could not save new event: " + event, t); //$NON-NLS-1$
			return Response.status(Status.BAD_REQUEST).entity(t).build();
		}
	}

}
