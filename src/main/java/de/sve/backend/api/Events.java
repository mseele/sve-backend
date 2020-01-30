package de.sve.backend.api;

import java.util.List;
import java.util.logging.Level;
import java.util.logging.Logger;
import java.util.stream.Collectors;

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
import de.sve.backend.model.Event;
import de.sve.backend.model.EventCounter;
import de.sve.backend.model.EventType;
import de.sve.backend.store.DataStore;

@Path("/events")
public class Events {

	private static final Logger LOG = Logger.getLogger(Events.class.getName());

	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<Event> getEvents() throws BackendException {
		try {
			return events();
		} catch (Throwable t) {
			String message = "Error while loading events"; //$NON-NLS-1$
			LOG.log(Level.SEVERE, message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/counters")
	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<EventCounter> getCounters() throws BackendException {
		try {
			return events().stream()
						   .map(EventCounter::create)
						   .collect(Collectors.toList());
		} catch (Throwable t) {
			String message = "Error while loading events"; //$NON-NLS-1$
			LOG.log(Level.SEVERE, message, t);
			throw new BackendException(message, t);
		}
	}

//	@Path("{e}")
//	@GET
//	@Produces(MediaType.APPLICATION_JSON)
//	public Event getEvent(@PathParam("e") String id) {
//		List<Event> events = events();
//		Event event = null;
//		for (Event e : events) {
//			if (id.equals(e.id())) {
//				event = e;
//				break;
//			}
//		}
//		if (events.size() > 1) {
//			int index = events.indexOf(event);
//			Event previous = events.get(index > 0 ? index - 1 : events.size() - 1);
//			Event next = events.get(index == events.size() - 1 ? 0 : index + 1);
//			return new EventData(event, new EventsEvent(previous), new EventsEvent(next));
//		}
//		return new EventData(event);
//	}
//
//	@Path("/booking")
//	@POST
//	@Consumes(MediaType.APPLICATION_JSON)
//	@Produces(MediaType.APPLICATION_JSON)
//	public BookingResponse booking(Booking booking) {
//		try {
//			Long id = Utils.fromHash(booking.id);
//			Event event = DataStore.getEvent(id);
//			if (event.subscribers < event.maxSubscribers) {
//				event.subscribers++;
//				return successfullBooking(booking, event, true);
//			} else if (event.waitingList < event.maxWaitingList) {
//				event.waitingList++;
//				return successfullBooking(booking, event, false);
//			}
//			LOG.log(Level.SEVERE, "Booking failed because Event (" + event.id + ") was overbooked."); //$NON-NLS-1$ //$NON-NLS-2$
//			String message = "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal."; //$NON-NLS-1$
//			return new BookingResponse(message);
//		} catch (Throwable t) {
//			LOG.log(Level.SEVERE, "Booking failed", t); //$NON-NLS-1$
//			String message = "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal."; //$NON-NLS-1$
//			return new BookingResponse(message);
//		}
//	}
//
//	private static BookingResponse successfullBooking(Booking booking, Event event, boolean isBooking) throws Throwable {
//		String message;
//		if (isBooking) {
//			message = "Die Buchung war erfolgreich. Du bekommst innerhalb von 24 Stunden eine Bestätigung per E-Mail."; //$NON-NLS-1$
//		} else {
//			message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden."; //$NON-NLS-1$
//		}
//		String result = confirmBooking(event, booking, isBooking);
////		try {
////			Mailjet.manualBooking(booking, event, isBooking);
////		} catch (Throwable t) {
////			// TODO: remove
////			Utils.sendEmail(booking, event, isBooking);
////		}
//		DataStore.saveEvent(event);
//		LOG.log(Level.INFO, "Booking of Event (" + event.id + ") was successfull: " + result); //$NON-NLS-1$ //$NON-NLS-2$
//		if (Utils.subscribeUpdates(booking)) {
//			Mailjet.subscribe(booking.email);
//		}
//		return new BookingResponse(message, new EventData(event));
//	}
//
//	@Path("/confirm_booking")
//	@GET
//	public Response confirmBooking(@QueryParam("b") boolean isBooking, @QueryParam("json") String json) {
//		try {
//			Booking booking = Utils.gson().fromJson(json, Booking.class);
//			Long id = Utils.fromHash(booking.id);
//			Event event = DataStore.getEvent(id);
//			String result = confirmBooking(event, booking, isBooking);
//			return Response.status(Status.OK).entity("Booking confirmed\n" + result).build(); //$NON-NLS-1$
//		} catch (Throwable t) {
//			LOG.log(Level.SEVERE, "Booking confirmation failed", t); //$NON-NLS-1$
//			return Response.status(Status.BAD_REQUEST).entity(t).build();
//		}
//	}
//
//	private static String confirmBooking(Event event, Booking booking, boolean isBooking) throws Throwable {
//		String returnValue = Spreadsheets.booking(booking, event);
//		Mailjet.automaticBooking(booking, event, isBooking);
//		return returnValue;
//	}
//
//	@Path("/message")
//	@POST
//	@Consumes(MediaType.APPLICATION_JSON)
//	public Response message(Message message) {
//		try {
//			Mailjet.sendEmail(message);
//			return Response.status(Status.OK).build();
//		} catch (Throwable t) {
//			LOG.log(Level.SEVERE, "Could not save new event", t); //$NON-NLS-1$
//			return Response.status(Status.BAD_REQUEST).entity(t).build();
//		}
//	}

	@Path("/update")
	@POST
	@Consumes(MediaType.APPLICATION_JSON)
	public Response update(Event event) {
		try {
			DataStore.save(event);
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
			DataStore.delete(event);
			LOG.log(Level.INFO, "Event (" + event.id() + ") has been deleted"); //$NON-NLS-1$ //$NON-NLS-2$
			return Response.status(Status.OK).build();
		} catch (Throwable t) {
			LOG.log(Level.SEVERE, "Could not save new event", t); //$NON-NLS-1$
			return Response.status(Status.BAD_REQUEST).entity(t).build();
		}
	}

	public static List<Event> events() throws Exception {
		return events(null);
	}

	public static List<Event> events(EventType type) throws Exception {
		return DataStore.events()
				.stream()
		 		.filter(e -> e.visible() && (type == null || type == e.type()))
		 		.sorted((event1, event2) -> {
					 if (event1.isBookedUp() == event2.isBookedUp()) {
						 return event1.sortIndex().compareTo(event2.sortIndex());
					 } else if (event1.isBookedUp()) {
						 return 1;
					 }
					 return -1;
				 })
		 		.collect(Collectors.toList());
	}

}
