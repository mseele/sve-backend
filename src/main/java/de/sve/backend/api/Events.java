package de.sve.backend.api;

import java.util.List;
import java.util.logging.Level;
import java.util.logging.Logger;

import javax.ws.rs.Consumes;
import javax.ws.rs.GET;
import javax.ws.rs.POST;
import javax.ws.rs.Path;
import javax.ws.rs.Produces;
import javax.ws.rs.core.MediaType;
import javax.ws.rs.core.Response;
import javax.ws.rs.core.Response.Status;

import de.sve.backend.Utils;
import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.EventsManager;
import de.sve.backend.model.events.BookingResponse;
import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventBooking;
import de.sve.backend.model.events.EventCounter;

@Path("/events")
public class Events {

	private static final Logger LOG = Logger.getLogger(Events.class.getName());

	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<Event> getEvents() throws BackendException {
		try {
			return EventsManager.events();
		} catch (Throwable t) {
			String message = "Error while loading events"; //$NON-NLS-1$
			LOG.log(Level.SEVERE, message, t);
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
			LOG.log(Level.SEVERE, message, t);
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

	@Path("/update")
	@POST
	@Consumes(MediaType.APPLICATION_JSON)
	public Response update(Event event) {
		try {
			EventsManager.update(event);
			LOG.log(Level.INFO, "Event (" + event.id() + ") has been updated"); //$NON-NLS-1$ //$NON-NLS-2$
			return Response.status(Status.OK).entity(Utils.gson().toJson(event)).build();
		} catch (Throwable t) {
			LOG.log(Level.SEVERE, "Could not save new event", t); //$NON-NLS-1$
			return Response.status(Status.BAD_REQUEST).entity(t).build();
		}
	}

	@Path("/delete")
	@POST
	@Consumes(MediaType.APPLICATION_JSON)
	public Response delete(Event event) {
		try {
			EventsManager.delete(event);
			LOG.log(Level.INFO, "Event (" + event.id() + ") has been deleted"); //$NON-NLS-1$ //$NON-NLS-2$
			return Response.status(Status.OK).build();
		} catch (Throwable t) {
			LOG.log(Level.SEVERE, "Could not save new event", t); //$NON-NLS-1$
			return Response.status(Status.BAD_REQUEST).entity(t).build();
		}
	}

}
